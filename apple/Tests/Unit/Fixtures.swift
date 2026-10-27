import Foundation
import SwimlieCore
import XCTest

/// Bundle anchor for the recordings copied in as folder resources (project.yml, SwimlieWatchTests).
final class Fixtures {}

/// Wall clock for the driver: tests set `now` by hand.
final class Clock: @unchecked Sendable {
    var now: Date
    init(_ now: Date) { self.now = now }
}

let t0 = Date(timeIntervalSince1970: 1_700_000_000) // swim start
let recordingNames = ["recorded-swim-a", "recorded-swim-b"]

func ms(_ d: Date) -> Int64 { Int64(d.timeIntervalSince1970 * 1000) }

func freshDefaults() -> UserDefaults {
    UserDefaults(suiteName: "swimlie.tests.\(UUID().uuidString)")!
}

@MainActor
func makeDriver(clock: Clock, defaults: UserDefaults) -> SwimDriver {
    SwimDriver(now: { clock.now }, defaults: defaults, feedback: SwimFeedback(retry: {}, tick: {}, go: {}))
}

/// One recording as the engine was fed it (core/tests/engine_replay.rs): laps from the first labelled one on, by receive time.
struct Recording {
    struct Entry {
        var lap: LapEvent
        var receivedS: Double
    }
    let workout: Workout
    let entries: [Entry]

    enum Failure: Error { case missing(String) }

    static let strokes: [String: Stroke] = [
        "any": .any, "freestyle": .freestyle, "backstroke": .backstroke, "breaststroke": .breaststroke,
        "butterfly": .butterfly, "mixed": .mixed, "kickboard": .kickboard,
    ]

    static func load(_ name: String) throws -> Recording {
        let bundle = Bundle(for: Fixtures.self)
        func text(_ file: String) throws -> String {
            guard let url = bundle.url(forResource: file, withExtension: "json", subdirectory: name) else { throw Failure.missing("\(name)/\(file).json") }
            return try String(contentsOf: url, encoding: .utf8)
        }
        let workout = try parseWorkout(json: text("workout"))
        let root = try JSONSerialization.jsonObject(with: Data(text("laps").utf8)) as! [String: Any]
        var laps = (root["laps"] as! [[String: Any]]).sorted { ($0["received_s"] as! Double) < ($1["received_s"] as! Double) }
        laps.removeFirst(laps.firstIndex { !($0["block"] is NSNull) }!) // unlabelled laps before the first labelled one were never fed
        let entries = laps.map { l in
            Entry(
                lap: LapEvent(
                    key: "lap-\(l["n"] as! Int)",
                    start: t0.addingTimeInterval(l["start_s"] as! Double),
                    end: t0.addingTimeInterval(l["end_s"] as! Double),
                    stroke: strokes[l["stroke"] as! String]!),
                receivedS: l["received_s"] as! Double)
        }
        return Recording(workout: workout, entries: entries)
    }
}

/// The engine and driver after a replay, with the clock they ended on.
@MainActor
struct Run {
    let clock: Clock
    let driver: SwimDriver
    let engine: LiveEngine
    let defaults: UserDefaults
}

/// Starts a swim and feeds the recording live: the clock moves to each lap's receive time before the lap arrives.
/// `pauseAfter`: index of a lap after which the swimmer presses pause and waits 90 s of wall clock. The laps keep their
/// timestamps (HealthKit has no laps while paused), so the timeline has no gap; only the clock moves.
@MainActor
func replay(_ r: Recording, pauseAfter k: Int? = nil) throws -> Run {
    let clock = Clock(t0)
    let defaults = freshDefaults()
    let driver = makeDriver(clock: clock, defaults: defaults)
    let engine = try LiveEngine(workout: r.workout, workoutId: "test", startedAtMs: ms(t0), records: [])
    driver.begin(engine: engine, workout: r.workout, id: "test", start: t0)
    for (i, en) in r.entries.enumerated() {
        clock.now = max(clock.now, t0 + en.receivedS) // the clock never goes back
        driver.tick()
        driver.feed(laps: [en.lap])
        if i == k {
            XCTAssertEqual(driver.newPauseRequests([PauseEvent(key: "p1"), PauseEvent(key: "p1")]), 1, "a double press in one batch counts once")
            XCTAssertEqual(driver.newPauseRequests([PauseEvent(key: "p1")]), 0, "a press seen before is ignored")
            let before = driver.state
            XCTAssertEqual(before?.phase, .swimming, "pause is tested mid-length")
            driver.setPaused(true)
            clock.now.addTimeInterval(90)
            driver.tick()
            XCTAssertEqual(driver.state?.pos, before?.pos, "a paused rep does not advance")
            XCTAssertEqual(driver.state?.doneM, before?.doneM, "a paused rep does not gain distance")
            driver.setPaused(false)
        }
    }
    return Run(clock: clock, driver: driver, engine: engine, defaults: defaults)
}
