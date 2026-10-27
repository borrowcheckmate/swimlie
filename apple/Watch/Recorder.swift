import CoreMotion
import Foundation
import HealthKit
import SwimlieCore
import WatchConnectivity
import WatchKit

/// Spike 0b: logs everything HealthKit and CoreMotion report during a pool swim to an NDJSON file.
final class Recorder: NSObject, ObservableObject, HKWorkoutSessionDelegate, HKLiveWorkoutBuilderDelegate, WCSessionDelegate, @unchecked Sendable {
    @Published var running = false
    @Published var elapsed: TimeInterval = 0
    @Published var distance: Double = 0
    @Published var laps = 0
    @Published var lapList: [Lap] = [] // seconds since startedAt; for freeSets
    @Published var startedAt = Date()
    @Published var poolLength = 25
    @Published var lastEvent = "-"
    @Published var heartRate: Double?
    @Published var pauses: [Pause] = []
    @Published var pausedNow = false

    struct Pause: Identifiable {
        let id = UUID()
        let start: Date
        var end: Date?
        var duration: TimeInterval { (end ?? Date()).timeIntervalSince(start) }
    }

    private let store = HKHealthStore()
    private var session: HKWorkoutSession?
    private var builder: HKLiveWorkoutBuilder?
    private let motion = CMMotionManager()
    private let motionQueue = OperationQueue()
    private var ticker: Timer?
    private var flushTimer: DispatchSourceTimer?

    // All file state is touched only on io.
    private let io = DispatchQueue(label: "recorder.io")
    private var handle: FileHandle?
    private var fileURL: URL?
    private var buffer = Data()
    private var loggedEvents = Set<String>() // type|start|end of events already written
    private var pauseList: [Pause] = [] // copied to pauses on main
    private var pauseSource: String? // "motion", "manual" or "button" while a pause is open
    private var actedEventKeys = Set<String>()
    private var sessionStart = Date() // lap times are offsets from this, set before any event can arrive

    override init() {
        super.init()
        if WCSession.isSupported() {
            WCSession.default.delegate = self
            WCSession.default.activate()
        }
    }

    // MARK: start / stop

    func start(poolLength: Int) {
        Task {
            do { try await begin(poolLength: poolLength) }
            catch {
                log("session", ["error": "\(error)"])
                await MainActor.run { self.lastEvent = String(localized: "Start failed: \(error.localizedDescription)") }
            }
        }
    }

    private func begin(poolLength: Int) async throws {
        let (session, builder) = try await PoolWorkout.make(store: store, poolLengthM: Double(poolLength))
        session.delegate = self
        builder.delegate = self
        self.session = session
        self.builder = builder

        openFile()
        // Motion timestamps count from boot; this pair converts them to wall time: wall + (t - uptime).
        log("clock", ["uptime": ProcessInfo.processInfo.systemUptime, "wall": Date().timeIntervalSince1970])
        // The sent workout (a plan's planned swim is sent as a workout), so a recording can be compared with what was planned.
        if let json = UserDefaults.standard.string(forKey: "sentWorkoutJSON"), !json.isEmpty {
            log("workout", ["id": UserDefaults.standard.string(forKey: "sentWorkoutId") ?? "", "workout": (try? JSONSerialization.jsonObject(with: Data(json.utf8))) ?? json])
        }
        let start = Date()
        sessionStart = start
        session.startActivity(with: start)
        try await builder.beginCollection(at: start)
        startMotion()

        await MainActor.run {
            let device = WKInterfaceDevice.current() // WatchKit traps off the main thread
            device.isBatteryMonitoringEnabled = true
            self.log("config", ["pool_length_m": poolLength, "device_model": device.model, "system_version": device.systemVersion, "battery": device.batteryLevel])
            self.elapsed = 0; self.distance = 0; self.laps = 0; self.lapList = []; self.startedAt = start; self.poolLength = poolLength; self.lastEvent = "-"
            self.heartRate = nil; self.pauses = []; self.pausedNow = false
            self.running = true
            self.ticker = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
                guard let self else { return }
                self.elapsed = self.builder?.elapsedTime ?? 0
                if Int(self.elapsed) % 60 == 0 { self.log("battery", ["level": WKInterfaceDevice.current().batteryLevel]) }
            }
        }
    }

    func stop() {
        Task {
            motion.stopDeviceMotionUpdates()
            session?.end()
            do {
                try await builder?.endCollection(at: Date())
                _ = try await builder?.finishWorkout()
            } catch {
                log("session", ["error": "\(error)"])
            }
            let battery = await MainActor.run { WKInterfaceDevice.current().batteryLevel }
            io.sync { self.closePause(at: Date()) } // a pause still open when the swim stops
            log("battery", ["level": battery])
            log("session", ["to": HKWorkoutSessionState.ended.rawValue, "date": Date().timeIntervalSince1970]) // queued before closeFile; the delegate may fire after it
            let url = closeFile()
            if let url, WCSession.default.activationState == .activated {
                WCSession.default.transferFile(url, metadata: nil)
            }
            await MainActor.run {
                self.ticker?.invalidate()
                self.running = false
            }
        }
    }

    // MARK: HealthKit delegates

    func workoutSession(_ s: HKWorkoutSession, didChangeTo to: HKWorkoutSessionState, from: HKWorkoutSessionState, date: Date) {
        log("session", ["from": from.rawValue, "to": to.rawValue, "date": date.timeIntervalSince1970])
        if to == .running { DispatchQueue.main.async { PoolWorkout.enableWaterLockIfWanted() } } // only works once the session is running; WatchKit traps off main
    }

    func workoutSession(_ s: HKWorkoutSession, didGenerate events: [HKWorkoutEvent]) {
        io.async { for e in events { self.trackPause(e) } }
    }

    func workoutSession(_ s: HKWorkoutSession, didFailWithError error: Error) {
        log("session", ["error": "\(error)"])
    }

    func workoutBuilderDidCollectEvent(_ b: HKLiveWorkoutBuilder) {
        // One callback can deliver several events, not always appended last, so log every unseen one.
        let events = b.workoutEvents
        io.async {
            let fresh = events.filter { self.loggedEvents.insert(PoolWorkout.key($0)).inserted }
            for e in fresh.sorted(by: { $0.dateInterval.start < $1.dateInterval.start }) {
                let start = e.dateInterval.start.timeIntervalSince1970
                let end = e.dateInterval.end.timeIntervalSince1970
                var meta: [String: String] = [:]
                var d: [String: Any] = ["type": e.type.rawValue, "type_name": Self.name(e.type), "start": start, "end": end]
                for (k, v) in e.metadata ?? [:] {
                    if k == HKMetadataKeySwimmingStrokeStyle, let n = v as? Int {
                        d["stroke_style"] = n
                        d["stroke_style_name"] = Self.strokeName(n)
                    } else {
                        meta[k] = "\(v)"
                    }
                }
                d["metadata"] = meta
                self.log("event", d)
                self.trackPause(e)
            }
        }
        let lapEvents = events.filter { $0.type == .lap } // workoutEvents is cumulative, so this is every lap so far
        let lapList = lapEvents.map { Lap(startS: $0.dateInterval.start.timeIntervalSince(sessionStart), endS: $0.dateInterval.end.timeIntervalSince(sessionStart), stroke: PoolWorkout.stroke(PoolWorkout.strokeStyle($0)), strokes: nil) }
        let label = events.last.map { Self.name($0.type) } ?? "-"
        DispatchQueue.main.async { self.laps = lapEvents.count; self.lapList = lapList; self.lastEvent = label }
    }

    private static func strokeName(_ n: Int) -> String {
        switch HKSwimmingStrokeStyle(rawValue: n) {
        case .unknown: "unknown"
        case .mixed: "mixed"
        case .freestyle: "freestyle"
        case .backstroke: "backstroke"
        case .breaststroke: "breaststroke"
        case .butterfly: "butterfly"
        case .kickboard: "kickboard"
        default: "unknown"
        }
    }

    func workoutBuilder(_ b: HKLiveWorkoutBuilder, didCollectDataOf types: Set<HKSampleType>) {
        for case let t as HKQuantityType in types {
            guard let s = b.statistics(for: t) else { continue }
            var d: [String: Any] = [
                "identifier": t.identifier,
                "start": s.startDate.timeIntervalSince1970, "end": s.endDate.timeIntervalSince1970,
            ]
            switch t.identifier {
            case HKQuantityTypeIdentifier.distanceSwimming.rawValue:
                if let v = s.sumQuantity()?.doubleValue(for: .meter()) {
                    d["sum_m"] = v
                    DispatchQueue.main.async { self.distance = v }
                }
            case HKQuantityTypeIdentifier.swimmingStrokeCount.rawValue:
                d["sum_count"] = s.sumQuantity()?.doubleValue(for: .count())
            case HKQuantityTypeIdentifier.activeEnergyBurned.rawValue:
                d["sum_kcal"] = s.sumQuantity()?.doubleValue(for: .kilocalorie())
            case HKQuantityTypeIdentifier.heartRate.rawValue:
                let bpm = s.mostRecentQuantity()?.doubleValue(for: HKUnit.count().unitDivided(by: .minute()))
                d["recent_bpm"] = bpm
                d["recent_at"] = s.mostRecentQuantityDateInterval()?.start.timeIntervalSince1970
                if let bpm { DispatchQueue.main.async { self.heartRate = bpm } }
            default: break
            }
            log("stats", d)
        }
    }

    private static func name(_ t: HKWorkoutEventType) -> String {
        switch t {
        case .lap: "lap"
        case .segment: "segment"
        case .marker: "marker"
        case .pause: "pause"
        case .resume: "resume"
        case .motionPaused: "motionPaused"
        case .motionResumed: "motionResumed"
        case .pauseOrResumeRequest: "pauseOrResumeRequest"
        @unknown default: "unknown"
        }
    }

    // MARK: pauses

    /// Pauses run from a pause event to the next resume. Call on io.
    private func trackPause(_ e: HKWorkoutEvent) {
        guard actedEventKeys.insert(PoolWorkout.key(e)).inserted else { return }
        let at = e.dateInterval.start
        switch e.type {
        case .motionPaused, .pause:
            guard pauseSource == nil else { return }
            pauseSource = e.type == .motionPaused ? "motion" : "manual"
            pauseList.append(Pause(start: at, end: nil))
            publishPauses()
        case .motionResumed, .resume:
            closePause(at: at)
        case .pauseOrResumeRequest: // crown + side button: toggle; the .pause/.resume events that follow find the pause already open or closed
            if session?.state == .running, pauseSource == nil {
                pauseSource = "button"
                pauseList.append(Pause(start: at, end: nil))
                publishPauses()
                session?.pause()
            } else if session?.state == .paused {
                closePause(at: at)
                session?.resume()
            }
        default: break
        }
    }

    /// Ends the open pause at `end` and logs it. Call on io.
    private func closePause(at end: Date) {
        guard let source = pauseSource, var p = pauseList.last else { return }
        pauseSource = nil
        p.end = end
        pauseList[pauseList.count - 1] = p
        log("pause", ["start": p.start.timeIntervalSince1970, "end": end.timeIntervalSince1970, "duration_s": p.duration, "source": source])
        publishPauses()
    }

    private func publishPauses() { // call on io
        let list = pauseList, now = pauseSource != nil
        DispatchQueue.main.async { self.pauses = list; self.pausedNow = now }
    }

    // MARK: motion

    private func startMotion() {
        guard motion.isDeviceMotionAvailable else { return }
        motion.deviceMotionUpdateInterval = 1.0 / 50
        motion.startDeviceMotionUpdates(to: motionQueue) { [weak self] m, _ in
            guard let m else { return }
            self?.log("motion", [
                "t": m.timestamp,
                "ua": [m.userAcceleration.x, m.userAcceleration.y, m.userAcceleration.z],
                "rr": [m.rotationRate.x, m.rotationRate.y, m.rotationRate.z],
                "q": [m.attitude.quaternion.x, m.attitude.quaternion.y, m.attitude.quaternion.z, m.attitude.quaternion.w],
            ])
        }
    }

    // MARK: file

    private func openFile() {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let url = docs.appendingPathComponent("swim-\(ISO8601DateFormatter().string(from: Date())).ndjson")
        FileManager.default.createFile(atPath: url.path, contents: nil)
        io.sync {
            handle = try? FileHandle(forWritingTo: url)
            fileURL = url
            buffer = Data()
            loggedEvents = []
            pauseList = []; pauseSource = nil; actedEventKeys = []
        }
        let t = DispatchSource.makeTimerSource(queue: io) // flush once per second: saves battery/IO
        t.schedule(deadline: .now() + 1, repeating: 1)
        t.setEventHandler { [weak self] in self?.flush() }
        t.resume()
        flushTimer = t
    }

    private func closeFile() -> URL? {
        flushTimer?.cancel()
        flushTimer = nil
        return io.sync {
            flush()
            try? handle?.close()
            handle = nil
            return fileURL
        }
    }

    private func flush() { // call on io
        guard !buffer.isEmpty else { return }
        try? handle?.write(contentsOf: buffer)
        buffer.removeAll(keepingCapacity: true)
    }

    private func log(_ kind: String, _ fields: [String: Any]) {
        var d = fields
        d["kind"] = kind
        d["received_at"] = Date().timeIntervalSince1970
        guard var line = try? JSONSerialization.data(withJSONObject: d) else { return }
        line.append(0x0A)
        io.async { self.buffer.append(line) }
    }

    // MARK: WCSessionDelegate

    func session(_ s: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) {
        if state == .activated { Sync.send() }
    }

    func session(_ s: WCSession, didReceive file: WCSessionFile) { _ = Sync.receive(file) }
    func session(_ s: WCSession, didFinish transfer: WCSessionFileTransfer, error: Error?) { Sync.finished(transfer) }
    func session(_ s: WCSession, didReceiveApplicationContext ctx: [String: Any]) { Sync.receiveWorkout(ctx) }
}
