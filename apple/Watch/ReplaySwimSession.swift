#if DEBUG
import Foundation
import Observation
import SwimlieCore

/// Simulator stand-in for SwimSession: a recorded swim (core/tests/data/<name>) fed through SwimDriver on a simulated clock.
/// The recording is read from this checkout by its compile-time path (`#filePath`), so nothing is bundled and Release never has it.
/// shortcut: the rest countdown plays no tones (SwimFeedback no-ops) because SwimDriver's countdown sleeps on the real clock; upgrade by giving the countdown the simulated clock.
@MainActor @Observable final class ReplaySwimSession: SwimSessionModel {
    /// The recording's own workout, the one its laps are labelled against.
    let recorded: Workout
    private(set) var saved: Session?
    private(set) var ended = false
    let saving = false // the replay saves synchronously in end(), so there is no saving state to show
    private(set) var error: String?
    private(set) var heartRate: Double?
    private(set) var paused = false
    let ageYears: UInt32? = 40

    var workout: Workout? { driver.workout }
    var state: State? { driver.state }
    var stateDate: Date { driver.stateDate }

    private let store: Store
    private let speed: Double // simulated seconds per real second
    private let startS: Double // simulated seconds fast-forwarded at start
    private let plan: [(t: Double, action: String)] // -script steps, by simulated second
    private var steps: [(t: Double, action: String)] = [] // steps not yet run
    private let laps: [(received: Double, event: LapEvent)] // by arrival, times relative to the replay start
    private let clock: SimClock
    private let feedback: SwimFeedback
    private var driver: SwimDriver // observed: a relaunch swaps it, and the views must follow
    @ObservationIgnored private var simS = 0.0 // simulated seconds since start
    @ObservationIgnored private var ticker: Task<Void, Never>?
    private static let base = Date(timeIntervalSince1970: 0) // the replay's start on the simulated clock

    /// `script`: `next@120,back@130,pause@200,resume@210,relaunch@300` (simulated seconds). `startS`: fast-forwarded to, before real time runs.
    init(store: Store, name: String, speed: Double, startS: Double, script: String) throws {
        let dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appending(path: "core/tests/data/\(name)")
        recorded = try parseWorkout(json: String(contentsOf: dir.appending(path: "workout.json"), encoding: .utf8))
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let all = try decoder.decode(Recording.self, from: Data(contentsOf: dir.appending(path: "laps.json"))).laps
        // Laps before the first labelled one are not fed: there is no record of that part of the workout (as in engine_replay.rs).
        guard let first = all.firstIndex(where: { $0.block != nil }) else { throw ReplayError.unlabelled }
        let origin = all[first].startS
        laps = all[first...].map { lap in
            (received: lap.receivedS - origin, event: LapEvent(key: "\(lap.n)", start: Self.base + (lap.startS - origin), end: Self.base + (lap.endS - origin), stroke: Self.stroke(lap.stroke)))
        }.sorted { $0.received < $1.received }
        self.store = store
        self.speed = speed
        self.startS = startS
        plan = script.split(separator: ",").compactMap { item in
            let parts = item.split(separator: "@")
            guard parts.count == 2, let t = Double(parts[1]) else { assertionFailure("replay -script: bad step \(item)"); return nil }
            return (t: t, action: String(parts[0]))
        }.sorted { $0.t < $1.t }
        let clock = SimClock()
        let feedback = SwimFeedback(retry: {}, tick: {}, go: {}) // shortcut: see the file comment
        self.clock = clock
        self.feedback = feedback
        driver = SwimDriver(now: { clock.now }, feedback: feedback)
    }

    func start(stored: StoredWorkout) {
        ticker?.cancel()
        saved = nil
        ended = false
        error = nil
        paused = false
        clock.now = Self.base
        simS = 0
        steps = plan
        driver = SwimDriver(now: { [clock] in clock.now }, feedback: feedback)
        driver.prepare(stored.workout)
        do {
            let e = try LiveEngine(workout: stored.workout, workoutId: stored.id, startedAtMs: Int64(Self.base.timeIntervalSince1970 * 1000), records: records())
            driver.begin(engine: e, workout: stored.workout, id: stored.id, start: Self.base)
        } catch {
            self.error = "\(error)"
            return
        }
        advance(toS: startS) // no waiting: the clock jumps, then the ticker runs in real time
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self else { return }
                self.advance(toS: self.simS + self.speed)
            }
        }
    }

    /// Advances simulated time to `t` seconds since the start, one second at a time (the watch's 1 Hz tick). The ticker calls it; tests and screenshots can call it directly.
    func advance(toS t: Double) {
        while simS < t { step() }
    }

    func manualNext() { driver.manualNext() }

    @discardableResult func manualBack() -> Bool { driver.manualBack() }

    func togglePause() { setPaused(!paused) }

    private func setPaused(_ p: Bool) {
        paused = p
        driver.setPaused(p)
    }

    func end() {
        ticker?.cancel()
        guard let engine = driver.engine else { return }
        driver.stopCountdown()
        do {
            var session = engine.session()
            session.ageYears = ageYears
            _ = try store.saveSession(id: nil, session: session)
            driver.clearPending()
            Sync.send()
            saved = session
            ended = true
            driver.release()
        } catch {
            self.error = "\(error)"
        }
    }

    private func step() {
        simS += 1
        clock.now = Self.base + simS
        while let s = steps.first, s.t <= simS { steps.removeFirst(); run(s.action) }
        driver.feed(laps: laps.filter { $0.received <= simS }.map(\.event))
        driver.tick()
        let bpm = 130 + 20 * sin(simS / 30) // drifts 110-150
        heartRate = bpm
        driver.onHeartRate(bpm)
    }

    private func run(_ action: String) {
        switch action {
        case "next": manualNext()
        case "back": manualBack()
        case "pause": setPaused(true)
        case "resume": setPaused(false)
        case "relaunch": relaunch()
        default: assertionFailure("replay -script: unknown action \(action)")
        }
    }

    /// What watchOS does on a relaunch mid-swim: a new driver rebuilt from the laps that have arrived and the saved marks.
    private func relaunch() {
        guard let p = driver.pending(startedAt: Self.base),
              let engine = try? LiveEngine(workout: recorded, workoutId: p.id, startedAtMs: Int64(Self.base.timeIntervalSince1970 * 1000), records: records())
        else { assertionFailure("replay relaunch: no saved swim"); return }
        driver.stopCountdown()
        let d = SwimDriver(now: { [clock] in clock.now }, feedback: feedback)
        d.recover(engine: engine, workout: recorded, start: Self.base, pauses: [], laps: laps.filter { $0.received <= simS }.map(\.event), marks: p.marks)
        if paused { d.setPaused(true) }
        driver = d
    }

    private func records() -> [PersonalRecord] { (try? store.personalRecords()) ?? [] }

    private static func stroke(_ s: String) -> Stroke {
        switch s {
        case "freestyle": .freestyle
        case "backstroke": .backstroke
        case "breaststroke": .breaststroke
        case "butterfly": .butterfly
        case "kickboard": .kickboard
        case "any": .any
        default: .mixed
        }
    }
}

private enum ReplayError: Error { case unlabelled }

/// The simulated clock the driver reads; only the replay moves it.
@MainActor private final class SimClock {
    var now = Date(timeIntervalSince1970: 0)
}

private struct Recording: Decodable {
    struct Lap: Decodable {
        let n: Int
        let receivedS: Double
        let startS: Double
        let endS: Double
        let stroke: String
        let block: String?
    }
    let laps: [Lap]
}
#endif
