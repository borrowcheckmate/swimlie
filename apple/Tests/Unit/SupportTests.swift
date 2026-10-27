import Foundation
import SwimlieCore
import XCTest

final class PendingTests: XCTestCase {
    func testSaveLoadClear() throws {
        let defaults = freshDefaults()
        XCTAssertNil(Pending.load(defaults))
        Pending(json: "{}", id: "w1", start: t0, marks: [.init(t: 12.5, laps: 4)]).save(defaults)
        let p = try XCTUnwrap(Pending.load(defaults))
        XCTAssertEqual(p.id, "w1")
        XCTAssertEqual(p.start, t0)
        XCTAssertEqual(p.marks.map { [$0.t, Double($0.laps)] }, [[12.5, 4]])
        Pending.clear(defaults)
        XCTAssertNil(Pending.load(defaults))
    }

    /// A relaunch only resumes the saved swim when HealthKit's start matches it (within 1 s).
    @MainActor func testPendingMatchesStartTime() {
        let defaults = freshDefaults()
        let clock = Clock(t0)
        let driver = makeDriver(clock: clock, defaults: defaults)
        Pending(json: "{}", id: "w1", start: t0).save(defaults)
        XCTAssertNotNil(driver.pending(startedAt: t0.addingTimeInterval(0.5)))
        XCTAssertNil(driver.pending(startedAt: t0.addingTimeInterval(2)))
    }
}

final class SyncTests: XCTestCase {
    /// Seeding is idempotent, skips invalid files, and a deleted sample stays deleted (tombstone).
    func testSeedKeepsDeletedSamplesDeleted() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("sync-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let good = dir.appendingPathComponent("spike.json")
        let bad = dir.appendingPathComponent("bad.json")
        let text = try String(contentsOf: try XCTUnwrap(Bundle(for: Fixtures.self).url(forResource: "workout", withExtension: "json", subdirectory: recordingNames[0])), encoding: .utf8)
        try text.write(to: good, atomically: true, encoding: .utf8)
        try "not a workout".write(to: bad, atomically: true, encoding: .utf8)

        let store = try Store.open(path: dir.appendingPathComponent("test.sqlite").path)
        Sync.seed(store, urls: [good, bad])
        Sync.seed(store, urls: [good, bad])
        let seeded = try store.listWorkouts()
        XCTAssertEqual(seeded.count, 1)

        _ = try store.deleteWorkout(id: seeded[0].id)
        Sync.seed(store, urls: [good, bad])
        XCTAssertEqual(try store.listWorkouts().count, 0)
    }
}

final class FormatTests: XCTestCase {
    func testTimeFormatting() {
        XCTAssertEqual(mmss(65.4), "1:05")
        XCTAssertEqual(mmss(0), "0:00")
        XCTAssertEqual(signedSeconds(-3.24), "−3.2 s") // real minus sign
        XCTAssertEqual(signedSeconds(1.44), "+1.4 s")
        XCTAssertEqual(pace(nil), "-")
        XCTAssertEqual(pace(97.5275), "1:38 /100m")
    }

    func testMetres() {
        XCTAssertEqual(metres(900), "900 m")
    }
}
