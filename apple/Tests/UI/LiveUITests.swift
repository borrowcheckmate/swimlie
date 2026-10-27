import XCTest

/// The watch app in replay mode (DEBUG, `-replay` in ReplaySwimSession.swift): a recorded swim, sped up with `-speed`.
/// Simulated time cannot be jumped from here, so each test waits on elements, with timeouts sized to the speed.
final class LiveUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launch(speed: String, script: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-replay", "recorded-swim-b", "-speed", speed] + (script.map { ["-script", $0] } ?? [])
        app.launch()
        return app
    }

    /// Screenshot kept in the result bundle; xtask exports it to build/ui-attachments.
    private func shot(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// The live screen shows its rep counter, and a rest timer comes up during the swim.
    func testLiveShowsRepCounterAndRest() {
        let app = launch(speed: "10")
        let rep = app.staticTexts.matching(identifier: "live.rep").matching(NSPredicate(format: "label BEGINSWITH %@", "2/")).firstMatch
        XCTAssertTrue(rep.waitForExistence(timeout: 90))
        // The water fills by metres (half way at 25/50 m); give it a second to rise.
        XCTAssertTrue(app.staticTexts["25/50 m"].waitForExistence(timeout: 60))
        Thread.sleep(forTimeInterval: 1.5)
        shot(app, "live-rep")
        XCTAssertTrue(app.staticTexts["live.rest"].waitForExistence(timeout: 60))
        shot(app, "live-rest")
    }

    /// The first `next` is the start (GO) of the countdown, the second marks rep 1 done: the counter must read 2/N.
    /// Both run before the first labelled lap, so no natural lap can make the check pass by itself.
    func testScriptedNextAdvancesRep() {
        let app = launch(speed: "1", script: "next@1,next@3")
        // Matched by label too: a screen transition briefly has two rep counters on screen.
        let rep = app.staticTexts.matching(identifier: "live.rep").matching(NSPredicate(format: "label BEGINSWITH %@", "2/")).firstMatch
        if !rep.waitForExistence(timeout: 30) {
            shot(app, "no-advance")
            let labels = app.staticTexts.matching(identifier: "live.rep").allElementsBoundByIndex.map(\.label)
            return XCTFail("rep counter never reached 2/, it reads \(labels)")
        }
        shot(app, "after-next")
    }

    /// The recording ends by itself (about 5400 simulated s, so ~30 s at speed 200); End then shows the totals.
    func testSummaryShowsTotalsAfterRecordingEnds() {
        let app = launch(speed: "200")
        XCTAssertTrue(app.staticTexts["Workout done"].waitForExistence(timeout: 120))
        shot(app, "done")
        app.buttons["End"].firstMatch.tap()
        XCTAssertTrue(app.staticTexts["summary.distance"].waitForExistence(timeout: 30))
        XCTAssertTrue(app.staticTexts["summary.total_time"].exists)
        shot(app, "summary")
    }
}
