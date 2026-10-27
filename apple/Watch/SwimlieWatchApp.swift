import SwimlieCore
import SwiftUI
import WatchKit

final class AppDelegate: NSObject, WKApplicationDelegate {
    /// Set by the App init; rebuilds the live swim when watchOS relaunches the app mid-workout.
    @MainActor static var recover: (() async -> Void)?
    func handleActiveWorkoutRecovery() { Task { @MainActor in await Self.recover?() } }
}

@main
struct SwimlieWatchApp: App {
    @WKApplicationDelegateAdaptor private var appDelegate: AppDelegate
    @StateObject private var recorder = Recorder() // init activates WCSession at launch
    private let store: Store
    private let session: any SwimSessionModel

    init() {
        let dir = URL.applicationSupportDirectory
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        // A missing store is unrecoverable on a standalone watch app.
        let store = try! Store.open(path: dir.appending(path: "swimlie.sqlite").path)
        Sync.store = store
        Sync.seed(store, urls: Bundle.main.urls(forResourcesWithExtension: "json", subdirectory: nil) ?? [])
        self.store = store
        #if targetEnvironment(simulator)
        session = Self.simulatorSession(store)
        #else
        let real = SwimSession(store: store)
        session = real
        AppDelegate.recover = { await real.recover() }
        #endif
        #if DEBUG
        Self.demo(session, store)
        #endif
    }

    #if targetEnvironment(simulator)
    /// The simulator has no swim laps: a recorded swim (`-replay <folder>`, `-speed N`, `-script ...`; DEBUG), started at launch.
    /// `-demo` screenshots and Release builds use FakeSwimSession's synthetic laps.
    private static func simulatorSession(_ store: Store) -> any SwimSessionModel {
        #if DEBUG
        let d = UserDefaults.standard
        if d.string(forKey: "demo") == nil {
            let replay = try! ReplaySwimSession(store: store, name: d.string(forKey: "replay") ?? "recorded-swim-b",
                                                speed: d.string(forKey: "speed").flatMap(Double.init) ?? 5,
                                                startS: d.string(forKey: "start").flatMap(Double.init) ?? 0, script: d.string(forKey: "script") ?? "")
            replay.start(stored: StoredWorkout(id: "replay", createdAtMs: 0, updatedAtMs: 0, workout: replay.recorded))
            return replay
        }
        #endif
        return FakeSwimSession(store: store)
    }
    #endif

    #if DEBUG
    /// `-demo drill`: swim the sent workout (`-sentWorkoutJSON`). `-demo detail|upnext|swim|rest|pr|summary|saved|saved-bottom` (`recorder`: the Recorder screen, see body): jump to a screen with the first seeded workout (simctl cannot tap).
    private static func demo(_ session: any SwimSessionModel, _ store: Store) {
        if UserDefaults.standard.string(forKey: "demo") == "drill", let fake = session as? FakeSwimSession, // the sent workout (-sentWorkoutJSON) swimming: a drill's cue on the live screen
           let json = UserDefaults.standard.string(forKey: "sentWorkoutJSON"), let w = try? parseWorkout(json: json) {
            fake.start(stored: StoredWorkout(id: "drill", createdAtMs: 0, updatedAtMs: 0, workout: w))
            Task { @MainActor in fake.fastForward(toS: 20) }
            return
        }
        guard let fake = session as? FakeSwimSession, let name = UserDefaults.standard.string(forKey: "demo"),
              !name.hasPrefix("detail"), !name.hasPrefix("picker"), name != "recorder", let ws = try? store.listWorkouts(), let w = ws.first(where: { $0.workout.blocks.first?.sets.first?.rest != nil }) ?? ws.first else { return }
        fake.start(stored: w)
        let t: Double = ["upnext": 3, "swim": 20, "rest": 50, "pr": 110, "summary": 400, "saved": 400, "saved-bottom": 400][name] ?? 0
        Task { @MainActor in
            // The simulated fix arrives after start: wait for it (up to ~9 s) so a saved swim has its location.
            var waits = 0
            while UserDefaults.standard.bool(forKey: "saveLocation"), fake.locator.location == nil, waits < 30 { try? await Task.sleep(for: .milliseconds(300)); waits += 1 }
            fake.fastForward(toS: t)
            if name == "summary" || name.hasPrefix("saved") { fake.end() } // the summary of the saved swim
        }
    }
    #endif

    var body: some Scene {
        WindowGroup {
            #if DEBUG
            if UserDefaults.standard.string(forKey: "demo") == "recorder" { NavigationStack { RecorderView(recorder: recorder) } } // screenshot of the idle recorder
            else if UserDefaults.standard.bool(forKey: "bigtext") { root.dynamicTypeSize(.accessibility2) } // -bigtext YES: simctl cannot set the text size on watchOS
            else if UserDefaults.standard.bool(forKey: "dimmed") { root.environment(\.isLuminanceReduced, true) } else { root } // screenshots of the Always On look
            #else
            root
            #endif
        }
    }

    private var root: some View {
        RootView(store: store, session: session, recorderView: AnyView(RecorderView(recorder: recorder)))
    }
}
