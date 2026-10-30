import Foundation
import SwimlieCore

/// Simulator stand-in for SwimSession: a real LiveEngine fed with synthetic laps, no HealthKit.
@MainActor @Observable final class FakeSwimSession: SwimSessionModel {
    private(set) var workout: Workout?
    private(set) var state: State?
    private(set) var saved: Session?
    private(set) var ended = false
    let saving = false // the fake saves synchronously in end(), so there is no saving state to show
    private(set) var error: String?
    private(set) var heartRate: Double? = 130
    private(set) var paused = false
    var stateDate: Date { Date() }
    let ageYears: UInt32? = 40

    private let store: Store
    let locator = SwimLocator() // internal for the DEBUG demo, which waits for the fix
    @ObservationIgnored private var engine: LiveEngine?
    @ObservationIgnored private var ticker: Task<Void, Never>?
    @ObservationIgnored private var nowS = 0.0 // simulated seconds since start
    @ObservationIgnored private var lapStartS = 0.0
    private let speed = 5.0 // simulated seconds per real second
    @ObservationIgnored private var laps = 0
    /// Simulated seconds per 25 m length; varies, so in the quick test the second rep is a PR (`-demo pr`).
    private var lengthS: Double { 20 + 2 * sin(Double(laps) * 2.1) }

    init(store: Store) { self.store = store }

    func start(stored: StoredWorkout) {
        let workout = stored.workout
        ticker?.cancel()
        saved = nil
        ended = false
        error = nil
        nowS = 0
        lapStartS = 0
        laps = 0
        locator.request()
        do {
            let engine = try LiveEngine(workout: workout, workoutId: stored.id, startedAtMs: Int64(Date().timeIntervalSince1970 * 1000), records: (try? store.personalRecords()) ?? [])
            self.engine = engine
            self.workout = workout
            state = engine.tick(nowS: 0)
        } catch {
            self.error = "\(error)"
            return
        }
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                self?.step()
            }
        }
    }

    func manualNext() {
        guard let engine else { return }
        publish(engine.manualNext(nowS: nowS), after: state) { state = $0 }
    }

    @discardableResult func manualBack() -> Bool {
        guard let engine, let s = engine.manualBack(nowS: nowS) else { return false }
        publish(s, after: state) { state = $0 }
        return true
    }

    func togglePause() { paused.toggle() }

    func end() {
        ticker?.cancel()
        guard let engine else { return }
        self.engine = nil // a second end() must not save twice
        do {
            var session = engine.session()
            session.ageYears = ageYears
            session.location = locator.location
            _ = try store.saveSession(id: nil, session: session)
            Sync.send()
            saved = session
            ended = true
        } catch {
            self.error = "\(error)"
        }
    }

#if DEBUG
    /// Screenshot demo: jump to simulated time `t` and freeze there.
    func fastForward(toS t: Double) {
        while nowS < t { step() }
        ticker?.cancel()
    }
#endif

    private func step() {
        guard let engine else { return }
        nowS += speed
        let bpm = 130 + 20 * sin(nowS / 30) // drifts 110-150
        heartRate = bpm
        engine.onHeartRate(tS: nowS, bpm: bpm)
        var s = engine.tick(nowS: nowS)
        if s.phase == .swimming {
            // Swim a length whenever one has been completed since the last lap; rests just skip this.
            while lapStartS + lengthS <= nowS {
                let lap = Lap(startS: lapStartS, endS: lapStartS + lengthS, stroke: .freestyle, strokes: 18)
                lapStartS += lengthS
                laps += 1
                s = engine.onLap(lap: lap, nowS: lapStartS)
                if s.phase != .swimming { break }
            }
        } else {
            lapStartS = nowS // resume swimming from the end of the rest
        }
        publish(s, after: state) { state = $0 }
    }
}
