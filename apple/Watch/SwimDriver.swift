import Foundation
import Observation
import SwimlieCore

/// A lap as HealthKit reported it, in plain values. `key` dedupes: workoutEvents is cumulative.
struct LapEvent {
    var key: String
    var start: Date
    var end: Date
    var stroke: Stroke
}

/// A crown + side button press, in plain values. `key` dedupes, as for laps.
struct PauseEvent {
    var key: String
}

/// Haptic and tones the driver asks for; SwimSession supplies them.
struct SwimFeedback {
    var retry: () -> Void // double tap
    var tick: () -> Void // rest countdown at 3, 2, 1 s left
    var go: () -> Void // rest over
}

/// The swim in progress, kept so a relaunched app can rebuild the engine (HealthKit has the laps, not the manual marks).
struct Pending: Codable {
    var json: String
    var id: String
    var start: Date
    // Each manual next and back: seconds since start and laps fed by then, so a replay sees what the swimmer saw.
    var marks: [Mark] = []
    struct Mark: Codable { var t: Double; var laps: Int; var back: Bool? = nil } // back: nil or false is a next
    private static let key = "swimlie.pending"
    static func load(_ defaults: UserDefaults) -> Pending? { defaults.data(forKey: key).flatMap { try? JSONDecoder().decode(Pending.self, from: $0) } }
    func save(_ defaults: UserDefaults) { defaults.set(try? JSONEncoder().encode(self), forKey: Self.key) }
    static func clear(_ defaults: UserDefaults) { defaults.removeObject(forKey: key) }
}

/// Swim logic without HealthKit: the LiveEngine, lap and pause de-duplication, manual marks, recovery replay and the rest countdown.
/// All times are seconds since `start`.
@MainActor @Observable final class SwimDriver {
    private(set) var workout: Workout?
    private(set) var state: State?
    private(set) var stateDate = Date()
    @ObservationIgnored private(set) var start = Date()
    @ObservationIgnored private(set) var engine: LiveEngine?
    @ObservationIgnored private var seen = Set<String>()
    @ObservationIgnored private var seenPauses = Set<String>() // separate: seen.count is the lap count marks are replayed by
    @ObservationIgnored private var countdown: Task<Void, Never>? // rest countdown: tone at 3, 2, 1 s left; higher tone at 0
    @ObservationIgnored private var countdownPos: Position? // the rest that countdown belongs to
    @ObservationIgnored private let now: () -> Date
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private let feedback: SwimFeedback

    init(now: @escaping () -> Date = { Date() }, defaults: UserDefaults = .standard, feedback: SwimFeedback) {
        self.now = now
        self.defaults = defaults
        self.feedback = feedback
    }

    private var nowS: Double { now().timeIntervalSince(start) }

    /// The workout is chosen; its engine comes later with `begin` or `recover`.
    func prepare(_ workout: Workout) {
        self.workout = workout
        state = nil // the previous swim's state may not fit this workout
    }

    /// A new swim has its engine: show its first state and persist it for recovery.
    func begin(engine: LiveEngine, workout: Workout, id: String, start: Date) {
        attach(engine, start: start)
        apply(engine.tick(nowS: 0)) // the live screen shows before HealthKit collection starts
        Pending(json: workoutToJson(workout: workout), id: id, start: start).save(defaults)
    }

    /// The app was relaunched mid-swim: rebuild from HealthKit's laps and pauses plus the saved marks.
    func recover(engine: LiveEngine, workout: Workout, start: Date, pauses: [PauseEvent], laps: [LapEvent], marks: [Pending.Mark]) {
        self.workout = workout
        state = nil
        attach(engine, start: start)
        seenPauses.formUnion(pauses.map(\.key)) // old presses were already acted on
        feed(laps: laps, replay: marks)
        apply(engine.tick(nowS: nowS))
    }

    /// The saved swim, if its start matches HealthKit's (within 1 s): marks are relative to the original start.
    func pending(startedAt begin: Date) -> Pending? {
        Pending.load(defaults).flatMap { abs(begin.timeIntervalSince($0.start)) < 1 ? $0 : nil }
    }

    private func attach(_ engine: LiveEngine, start: Date) {
        self.start = start
        seen = []; seenPauses = []; stopCountdown()
        self.engine = engine
    }

    /// Timer tick, 1 Hz.
    func tick() {
        if let e = engine { apply(e.tick(nowS: nowS)) }
    }

    func manualNext() {
        guard let e = engine else { return }
        let t = nowS
        let before = e.tick(nowS: t)
        let s = e.manualNext(nowS: t)
        // An ignored press (phase Done) leaves no mark, so a replay and manualBack stay in step with the engine.
        if s != before, var p = Pending.load(defaults) { p.marks.append(.init(t: t, laps: seen.count)); p.save(defaults) }
        stopCountdown()
        apply(s)
    }

    @discardableResult func manualBack() -> Bool {
        let t = nowS
        guard let e = engine, let s = e.manualBack(nowS: t) else { return false } // nil: nothing to undo, so nothing to replay
        if var p = Pending.load(defaults) { p.marks.append(.init(t: t, laps: seen.count, back: true)); p.save(defaults) }
        stopCountdown()
        apply(s)
        return true
    }

    /// Laps not yet fed, applied in end order. `replay` (recovery): laps are applied as of their end time, each mark after the laps that had arrived when it was pressed.
    func feed(laps events: [LapEvent], replay: [Pending.Mark]? = nil) {
        guard let engine else { return }
        var marks = replay?[...] ?? []
        for (i, e) in events.filter({ seen.insert($0.key).inserted })
            .sorted(by: { $0.end < $1.end }).enumerated() {
            let lap = Lap(
                startS: e.start.timeIntervalSince(start),
                endS: e.end.timeIntervalSince(start),
                stroke: e.stroke,
                strokes: nil) // filled from HealthKit's samples at End
            while let m = marks.first, m.laps <= i { replayMark(m, engine); marks.removeFirst() }
            apply(engine.onLap(lap: lap, nowS: replay == nil ? nowS : lap.endS))
        }
        for m in marks { replayMark(m, engine) }
    }

    /// Re-applies a saved manual press: a back undoes the engine's latest action, as it did when pressed.
    private func replayMark(_ m: Pending.Mark, _ engine: LiveEngine) {
        if m.back == true {
            if let s = engine.manualBack(nowS: m.t) { apply(s) }
        } else {
            apply(engine.manualNext(nowS: m.t))
        }
    }

    /// Number of pause presses not seen before; each one toggles the session once.
    func newPauseRequests(_ events: [PauseEvent]) -> Int {
        events.filter { seenPauses.insert($0.key).inserted }.count
    }

    func setPaused(_ paused: Bool) { engine?.setPaused(paused: paused, nowS: nowS) }
    func onActivity() { engine?.onActivity(tS: nowS) } // laps can lag; strokes show the swimmer is still going
    func onHeartRate(_ bpm: Double) { engine?.onHeartRate(tS: nowS, bpm: bpm) }

    func stopCountdown() { countdown?.cancel(); countdown = nil; countdownPos = nil }

    func clearPending() { Pending.clear(defaults) }

    /// The swim is saved: no more ticks or laps reach the engine.
    func release() { engine = nil }

    private func apply(_ s: State) {
        stateDate = now()
        let left = state?.restRemainingS // the rest as of the previous tick
        let old = state
        publish(s, after: state) { state = $0 }
        // Last length of the rep: a double tap. The lap that ends the second-to-last length arrives 5–9 s late, so this lands mid-length.
        if let o = old, let pool = workout?.poolLengthM, s.phase == .swimming, o.pos == s.pos, s.targetM > pool,
           o.doneM < s.targetM - pool, (s.targetM - pool)..<s.targetM ~= s.doneM {
            feedback.retry()
        }
        // The engine ticks at 1 Hz, so a tick that first sees <= 4 s left schedules the marks at their exact times.
        // A rest that ends by itself keeps its countdown so the final mark plays; skipped or paused rests cancel it (manualNext/manualBack cancel explicitly).
        let early = s.phase == .swimming && (left ?? .infinity) > 1.5 // a lap ended the rest early
        if countdownPos != s.pos || s.phase == .done || early || (s.phase == .resting && s.restRemainingS == nil) {
            stopCountdown()
        }
        guard s.phase == .resting, let rem = s.restRemainingS, rem <= 4, countdownPos == nil else { return }
        countdownPos = s.pos
        countdown = Task {
            var elapsed = 0.0 // seconds since the schedule began
            for mark in [3.0, 2, 1, 0] where mark <= rem { // marks already passed are skipped
                do { try await Task.sleep(for: .seconds(max(0, rem - mark - elapsed))) } catch { return }
                elapsed = rem - mark
                if mark > 0 { feedback.tick() } else { feedback.go() } // the rest is over: a higher tone
            }
            self.countdown = nil; self.countdownPos = nil // finished: a later rest may start its own countdown
        }
    }
}
