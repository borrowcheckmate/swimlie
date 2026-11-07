import SwimlieCore
import XCTest

@MainActor
final class SwimDriverTests: XCTestCase {
    func testRecoveryRestoresPauseWindows() throws {
        let clock = Clock(t0)
        let defaults = freshDefaults()
        let workout = try parseWorkout(json: #"{"version":1,"name":"test","pool_length_m":25,"blocks":[{"name":"set","sets":[{"reps":1,"distance_m":100}]}]}"#)
        let a = makeDriver(clock: clock, defaults: defaults)
        let engineA = try LiveEngine(workout: workout, workoutId: "test", startedAtMs: ms(t0), records: [])
        a.begin(engine: engineA, workout: workout, id: "test", start: t0)
        clock.now = t0 + 4; a.setPaused(true)
        clock.now = t0 + 64; a.setPaused(false)
        clock.now = t0 + 66; a.tick()
        let pending = try XCTUnwrap(Pending.load(defaults))
        let b = makeDriver(clock: clock, defaults: defaults)
        let engineB = try LiveEngine(workout: workout, workoutId: "test", startedAtMs: ms(t0), records: [])
        b.recover(engine: engineB, workout: workout, start: t0, pauses: [], laps: [], marks: pending.marks, pauseMarks: pending.pauses ?? [])
        XCTAssertEqual(b.state, a.state)
        XCTAssertEqual(b.state?.restRemainingS, 4)
    }
    /// Cumulative lap events (workoutEvents repeats every lap so far) must not count twice.
    func testDuplicateLapsAreIgnored() throws {
        for name in recordingNames {
            let r = try Recording.load(name)
            let once = try replay(r)
            // The same stream, but each delivery repeats every lap so far.
            let clock = Clock(t0)
            let defaults = freshDefaults()
            let driver = makeDriver(clock: clock, defaults: defaults)
            let engine = try LiveEngine(workout: r.workout, workoutId: "test", startedAtMs: ms(t0), records: [])
            driver.begin(engine: engine, workout: r.workout, id: "test", start: t0)
            for (i, en) in r.entries.enumerated() {
                clock.now = t0 + en.receivedS
                driver.tick()
                driver.feed(laps: r.entries[...i].map(\.lap))
            }
            XCTAssertGreaterThan(engine.session().laps.count, 0, name)
            XCTAssertEqual(engine.session(), once.engine.session(), name)
        }
    }

    /// A pause (crown + side button) neither ends the rep nor advances it, and a press counts once.
    func testPauseIsNotABreak() throws {
        for name in recordingNames {
            let r = try Recording.load(name)
            let k = 30 // a lap index mid-set; the replay asserts the swimmer is mid-length there
            let reference = try replay(r)
            let paused = try replay(r, pauseAfter: k)
            XCTAssertEqual(paused.engine.session().lapPositions, reference.engine.session().lapPositions, name)
            XCTAssertEqual(paused.engine.session().laps.count, reference.engine.session().laps.count, name)
        }
    }

    /// Activity callbacks can arrive after the sample that proves the swimmer resumed.
    func testActivityUsesHealthKitSampleTime() throws {
        let clock = Clock(t0)
        let driver = makeDriver(clock: clock, defaults: freshDefaults())
        let workout = try parseWorkout(json: #"{"version":1,"name":"test","pool_length_m":25,"blocks":[{"name":"set","sets":[{"reps":2,"distance_m":100}]}]}"#)
        let engine = try LiveEngine(workout: workout, workoutId: "test", startedAtMs: ms(t0), records: [])
        driver.begin(engine: engine, workout: workout, id: "test", start: t0)
        for i in 0..<4 {
            let start = Double(i * 12)
            _ = engine.onLap(lap: Lap(startS: start, endS: start + 10, stroke: .freestyle, strokes: nil), nowS: start + 15)
        }
        clock.now = t0 + 100
        XCTAssertEqual(engine.tick(nowS: 100).phase, .resting)
        driver.onActivity(at: t0 + 95)
        clock.now = t0 + 100
        driver.tick()
        XCTAssertEqual(driver.state?.phase, .swimming)
        XCTAssertEqual(driver.state?.repElapsedS, 5)
    }

    /// Relaunch mid-swim: rebuilding from the laps, pauses and saved marks gives the same engine and state as never stopping.
    func testRecoveryEqualsUninterruptedRun() throws {
        for name in recordingNames {
            let r = try Recording.load(name)
            let clock = Clock(t0)
            let defaults = freshDefaults()
            let a = makeDriver(clock: clock, defaults: defaults)
            let engineA = try LiveEngine(workout: r.workout, workoutId: "test", startedAtMs: ms(t0), records: [])
            a.begin(engine: engineA, workout: r.workout, id: "test", start: t0)
            let markAfter: Set<Int> = [9, 25, 26] // manual next pressed a second after these laps
            for (i, en) in r.entries.enumerated() {
                clock.now = t0 + en.receivedS
                a.tick()
                a.feed(laps: [en.lap])
                if markAfter.contains(i) {
                    clock.now.addTimeInterval(1)
                    a.manualNext()
                }
            }
            let end = t0 + r.entries.last!.receivedS + 1
            clock.now = end
            a.tick()
            let marks = try XCTUnwrap(Pending.load(defaults)).marks
            XCTAssertEqual(marks.count, markAfter.count, name)

            let engineB = try LiveEngine(workout: r.workout, workoutId: "test", startedAtMs: ms(t0), records: [])
            let b = makeDriver(clock: clock, defaults: defaults)
            b.recover(engine: engineB, workout: r.workout, start: t0, pauses: [], laps: r.entries.map(\.lap), marks: marks)
            XCTAssertEqual(engineB.session(), engineA.session(), name)
            XCTAssertEqual(b.state, a.state, name)
        }
    }

    /// Manual next records a mark with the laps seen so far; manual back removes the last one.
    func testMarksRecordAndPop() throws {
        let r = try Recording.load(recordingNames[0])
        let clock = Clock(t0)
        let defaults = freshDefaults()
        let d = makeDriver(clock: clock, defaults: defaults)
        let e = try LiveEngine(workout: r.workout, workoutId: "test", startedAtMs: ms(t0), records: [])
        d.begin(engine: e, workout: r.workout, id: "test", start: t0)

        func feed(_ range: Range<Int>) {
            for en in r.entries[range] {
                clock.now = t0 + en.receivedS
                d.tick()
                d.feed(laps: [en.lap])
            }
        }
        feed(0..<3)
        clock.now = t0 + r.entries[2].receivedS + 1
        d.manualNext()
        XCTAssertEqual(try XCTUnwrap(Pending.load(defaults)).marks.map(\.laps), [3])

        feed(3..<5)
        d.manualNext()
        XCTAssertEqual(try XCTUnwrap(Pending.load(defaults)).marks.map(\.laps), [3, 5])

        // A back is persisted too (flagged), so a relaunch replays the same sequence.
        XCTAssertTrue(d.manualBack())
        let marks = try XCTUnwrap(Pending.load(defaults)).marks
        XCTAssertEqual(marks.map(\.laps), [3, 5, 5])
        XCTAssertEqual(marks.map { $0.back ?? false }, [false, false, true])
    }

    /// Back marks survive a save and load, and saves from before `back` existed still decode as nexts.
    func testPendingMarksKeepBackFlag() throws {
        let defaults = freshDefaults()
        let p = Pending(json: "{}", id: "test", start: t0, marks: [.init(t: 1, laps: 3), .init(t: 2, laps: 5, back: true)])
        p.save(defaults)
        let loaded = try XCTUnwrap(Pending.load(defaults))
        XCTAssertEqual(loaded.marks.map(\.back), [nil, true])

        let old = #"{"json":"{}","id":"test","start":0,"marks":[{"t":1,"laps":3}]}"#
        defaults.set(Data(old.utf8), forKey: "swimlie.pending")
        let legacy = try XCTUnwrap(Pending.load(defaults))
        XCTAssertEqual(legacy.marks.count, 1)
        XCTAssertNotEqual(legacy.marks[0].back, true)
    }
}
