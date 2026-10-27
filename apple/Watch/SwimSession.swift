import Foundation
import HealthKit
import Observation
import SwimlieCore
import WatchKit

/// Real pool swim: HealthKit session + laps feed the SwimDriver (the logic). All times are seconds since the driver's `start`.
@MainActor @Observable
final class SwimSession: NSObject, SwimSessionModel, HKWorkoutSessionDelegate, HKLiveWorkoutBuilderDelegate {
    var workout: Workout? { driver.workout }
    var state: State? { driver.state }
    private(set) var saved: Session?
    private(set) var ended = false
    private(set) var error: String?
    private(set) var heartRate: Double?
    private(set) var ageYears: UInt32?
    private(set) var paused = false
    var stateDate: Date { driver.stateDate }

    @ObservationIgnored private let db: Store
    @ObservationIgnored private let hk = HKHealthStore()
    @ObservationIgnored private let driver: SwimDriver
    @ObservationIgnored private var hkSession: HKWorkoutSession?
    @ObservationIgnored private var builder: HKLiveWorkoutBuilder?
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private let log = SwimLog()
    @ObservationIgnored private let locator = SwimLocator() // shortcut: a relaunch mid-swim loses the fix, so that swim saves without a location

    init(store: Store) {
        db = store
        driver = SwimDriver(feedback: SwimFeedback(
            retry: { WKInterfaceDevice.current().play(.retry) },
            tick: { Beeper.tick() },
            go: { Beeper.go() }))
        super.init()
    }

    private var start: Date { driver.start }

    /// Whole years from HealthKit's date of birth; nil if none is set.
    private func dateOfBirthAge() -> UInt32? {
        (try? hk.dateOfBirthComponents()).flatMap { Calendar.current.date(from: $0) }.flatMap { Calendar.current.dateComponents([.year], from: $0, to: Date()).year }.map(UInt32.init)
    }

    func start(stored: StoredWorkout) {
        let workout = stored.workout
        driver.prepare(workout)
        error = nil
        saved = nil
        ended = false
        ageYears = dateOfBirthAge()
        locator.request()
        Task {
            do {
                // Show the live screen first: authorization and session set-up can take seconds.
                let begin = Date()
                let e = try LiveEngine(workout: workout, workoutId: stored.id, startedAtMs: Int64(begin.timeIntervalSince1970 * 1000), records: (try? db.personalRecords()) ?? [])
                driver.begin(engine: e, workout: workout, id: stored.id, start: begin)
                startTimer()
                log.open(workoutJSON: workoutToJson(workout: workout), id: stored.id)
                let (session, builder) = try await PoolWorkout.make(store: hk, poolLengthM: Double(workout.poolLengthM))
                attachHK(session, builder)
                session.startActivity(with: begin)
                try await builder.beginCollection(at: begin)
                Beeper.prepare()
            } catch {
                timer?.invalidate(); timer = nil // a failed start must not keep ticking
                self.error = "Start failed: \(error.localizedDescription)"
            }
        }
    }

    private func attachHK(_ session: HKWorkoutSession, _ builder: HKLiveWorkoutBuilder) {
        session.delegate = self
        builder.delegate = self
        hkSession = session
        self.builder = builder
    }

    private func startTimer() {
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { _ in // strong: the session lives as long as the app, end() invalidates
            MainActor.assumeIsolated { self.driver.tick() }
        }
    }

    /// The app was relaunched mid-swim (WKApplicationDelegate.handleActiveWorkoutRecovery): rebuild the engine from HealthKit's laps plus our saved marks.
    func recover() async {
        guard driver.engine == nil else { return }
        do {
            guard let session = try await hk.recoverActiveWorkoutSession() else { return }
            let builder = session.associatedWorkoutBuilder()
            guard let begin = builder.startDate, let p = driver.pending(startedAt: begin),
                  let w = try? parseWorkout(json: p.json),
                  let e = try? LiveEngine(workout: w, workoutId: p.id, startedAtMs: Int64(begin.timeIntervalSince1970 * 1000), records: (try? db.personalRecords()) ?? [])
            else { // nothing to rebuild from: keep the swim in Health only
                session.end()
                try await builder.endCollection(at: Date())
                _ = try await builder.finishWorkout()
                error = String(localized: "Swim recovered without its workout: saved to Health only")
                return
            }
            saved = nil
            ended = false
            ageYears = dateOfBirthAge()
            attachHK(session, builder)
            driver.recover(engine: e, workout: w, start: begin, pauses: pauseEvents(builder.workoutEvents), laps: lapEvents(builder.workoutEvents), marks: p.marks)
            PoolWorkout.enableWaterLockIfWanted() // already running, so no didChangeTo to do it
            startTimer()
            Beeper.prepare()
        } catch {
            self.error = "Recovery failed: \(error.localizedDescription)"
        }
    }

    func manualNext() { log.write("mark", ["back": false]); driver.manualNext() }

    @discardableResult func manualBack() -> Bool { log.write("mark", ["back": true]); return driver.manualBack() }

    func end() {
        guard let engine = driver.engine, let hkSession, let builder else {
            if driver.engine == nil, error != nil { ended = true } // start failed: let the live view close
            return
        }
        self.hkSession = nil // ending: a second end() (double tap, or the .ended delegate) must not save twice
        timer?.invalidate()
        timer = nil
        driver.stopCountdown()
        let endDate = Date()
        hkSession.end()
        Task {
            var errors: [String] = []
            do { // HK and DB saves are independent: a failure in one must not skip the other
                try await builder.endCollection(at: endDate)
                driver.feed(laps: lapEvents(builder.workoutEvents)) // a lap 5–9 s late at End
                _ = try await builder.finishWorkout()
            } catch { errors.append("Health save failed: \(error.localizedDescription)") }
            var session = engine.session()
            session.ageYears = ageYears
            session.location = locator.location
            do { try await fillStrokes(&session, until: endDate) } // sums of HealthKit's own counts; without them the laps just have no strokes
            catch { errors.append("Stroke counts failed: \(error.localizedDescription)") }
            try? await fillHeartRate(&session, until: endDate) // HealthKit has every sample, even ones collected before a relaunch; live ones stay on failure or none
            do { _ = try db.saveSession(id: nil, session: session); driver.clearPending(); Sync.send() }
            catch { errors.append("Database save failed: \(error.localizedDescription)") }
            saved = session
            ended = true
            let all = ([self.error] + errors).compactMap { $0 } // keeps a session failure message
            if !all.isEmpty { self.error = all.joined(separator: "; ") }
            log.write("session", ["to": HKWorkoutSessionState.ended.rawValue, "date": Date().timeIntervalSince1970])
            log.close()
            driver.release()
            self.builder = nil
        }
    }

    /// HealthKit's lap events as plain values; the stroke style is read here so the driver never sees HealthKit types.
    private func lapEvents(_ events: [HKWorkoutEvent]) -> [LapEvent] {
        events.filter { $0.type == .lap }.map {
            LapEvent(key: PoolWorkout.key($0), start: $0.dateInterval.start, end: $0.dateInterval.end, stroke: PoolWorkout.stroke(PoolWorkout.strokeStyle($0)))
        }
    }

    private func pauseEvents(_ events: [HKWorkoutEvent]) -> [PauseEvent] {
        events.filter { $0.type == .pauseOrResumeRequest }.map { PauseEvent(key: PoolWorkout.key($0)) }
    }

    /// Lap.strokes = sum of the stroke-count samples whose midpoint falls in the lap's time span (a sample straddling a boundary goes whole to one lap).
    private func fillStrokes(_ session: inout Session, until end: Date) async throws {
        let type = HKQuantityType(.swimmingStrokeCount)
        let samples = try await HKSampleQueryDescriptor(
            predicates: [.quantitySample(type: type, predicate: HKQuery.predicateForSamples(withStart: start, end: end))],
            sortDescriptors: []).result(for: hk)
        let mids = samples.map { ($0.startDate.timeIntervalSince(start) + $0.endDate.timeIntervalSince(start)) / 2 }
        for i in session.laps.indices {
            let l = session.laps[i]
            let n = zip(samples, mids).filter { $1 >= l.startS && $1 < l.endS }.reduce(0) { $0 + $1.0.quantity.doubleValue(for: .count()) }
            session.laps[i].strokes = n > 0 ? UInt32(n.rounded()) : nil
        }
    }

    /// Replaces the live heart rate with HealthKit's samples from this swim (`tS` relative to `start`, in time order).
    private func fillHeartRate(_ session: inout Session, until end: Date) async throws {
        let type = HKQuantityType(.heartRate)
        let samples = try await HKSampleQueryDescriptor(
            predicates: [.quantitySample(type: type, predicate: HKQuery.predicateForSamples(withStart: start, end: end))],
            sortDescriptors: []).result(for: hk)
        let hr = samples.map { HrSample(tS: $0.startDate.timeIntervalSince(start), bpm: $0.quantity.doubleValue(for: HKUnit.count().unitDivided(by: .minute()))) }
            .filter { $0.tS >= 0 }.sorted { $0.tS < $1.tS }
        if !hr.isEmpty { session.hr = hr }
    }

    func togglePause() {
        guard let hkSession else { return }
        if hkSession.state == .paused { hkSession.resume() } else if hkSession.state == .running { hkSession.pause() }
    }

    /// Crown + side button: each new press toggles the HealthKit session.
    private func pauseRequests(_ events: [HKWorkoutEvent]) {
        guard hkSession != nil else { return }
        for _ in 0..<driver.newPauseRequests(pauseEvents(events)) { togglePause() }
    }

    // MARK: HealthKit delegates (arbitrary queue)

    nonisolated func workoutBuilderDidCollectEvent(_ b: HKLiveWorkoutBuilder) {
        let events = b.workoutEvents
        log.events(events)
        Task { @MainActor in self.driver.feed(laps: self.lapEvents(events)); self.pauseRequests(events) }
    }

    nonisolated func workoutBuilder(_ b: HKLiveWorkoutBuilder, didCollectDataOf types: Set<HKSampleType>) {
        if types.contains(HKQuantityType(.swimmingStrokeCount)) || types.contains(HKQuantityType(.distanceSwimming)) {
            Task { @MainActor in self.driver.onActivity() } // laps can lag; strokes show the swimmer is still going
        }
        guard types.contains(HKQuantityType(.heartRate)),
              let bpm = b.statistics(for: HKQuantityType(.heartRate))?.mostRecentQuantity()?.doubleValue(for: HKUnit.count().unitDivided(by: .minute())) else { return }
        Task { @MainActor in self.heartRate = bpm; self.driver.onHeartRate(bpm) }
    }

    nonisolated func workoutSession(_ s: HKWorkoutSession, didChangeTo to: HKWorkoutSessionState, from: HKWorkoutSessionState, date: Date) {
        log.write("session", ["from": from.rawValue, "to": to.rawValue, "date": date.timeIntervalSince1970])
        Task { @MainActor in
            if to == .running { PoolWorkout.enableWaterLockIfWanted() } // only works once the session is running
            if to == .paused || (to == .running && from == .paused) {
                self.paused = to == .paused
                self.driver.setPaused(self.paused)
            }
            if to == .ended { self.end() } // ended by the system: still save the swim
        }
    }

    // The crown + side button press is delivered here, not in builder.workoutEvents.
    nonisolated func workoutSession(_ s: HKWorkoutSession, didGenerate events: [HKWorkoutEvent]) {
        Task { @MainActor in self.pauseRequests(events) }
    }

    nonisolated func workoutSession(_ s: HKWorkoutSession, didFailWithError error: Error) {
        Task { @MainActor in // the session is unusable: save what we have
            self.error = "Workout session failed: \(error.localizedDescription)"
            self.end()
        }
    }
}
