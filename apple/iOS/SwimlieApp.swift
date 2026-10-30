import SwimlieCore
import SwiftUI

@main
struct SwimlieApp: App {
    @StateObject private var receiver: Receiver // init activates WCSession at launch
    @StateObject private var library = Library()
    @SwiftUI.State private var selection = Tab.workouts
    @AppStorage("debug") private var debug = false

    enum Tab: Hashable { case workouts, history, files, settings }

    init() {
        let dir = URL.applicationSupportDirectory
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let store = try! Store.open(path: dir.appending(path: "swimlie.sqlite").path)
        Sync.store = store // before Receiver, which can get files
        Sync.seed(store, urls: Bundle.main.url(forResource: "css-test", withExtension: "json").map { [$0] } ?? [])
        _receiver = StateObject(wrappedValue: Receiver())
        #if DEBUG
        if UserDefaults.standard.string(forKey: "demo") == "swim", let store = Sync.store { Demo.seedFromTestData(store) }
        else if let d = UserDefaults.standard.string(forKey: "demo"), !d.hasSuffix("-empty"), let store = Sync.store { Demo.seed(store) }
        let demo = UserDefaults.standard.string(forKey: "demo")
        if demo == "history" || demo == "swim" || demo?.hasPrefix("detail") == true || demo?.hasPrefix("trends") == true { _selection = SwiftUI.State(initialValue: .history) }
        if demo == "settings-ai" { _selection = SwiftUI.State(initialValue: .settings) }
        #endif
    }

    var body: some Scene {
        WindowGroup {
            TabView(selection: $selection) {
                LibraryView(library: library)
                    .tabItem { Label("Workouts", systemImage: "list.bullet") }
                    .tag(Tab.workouts)
                HistoryView()
                    .tabItem { Label("History", systemImage: "clock") }
                    .tag(Tab.history)
                if debug {
                    SwimFilesView(receiver: receiver)
                        .tabItem { Label("Swim files", systemImage: "folder") }
                        .tag(Tab.files)
                }
                NavigationStack { SettingsView() }
                    .tabItem { Label("Settings", systemImage: "gearshape") }
                    .tag(Tab.settings)
            }
            .onOpenURL { library.importFile($0) }
            .preferredColorScheme(.dark)
            .tint(Metric.distance.color)
        }
    }
}

struct SwimFilesView: View {
    @ObservedObject var receiver: Receiver

    var body: some View {
        NavigationStack {
            List {
                ForEach(receiver.files, id: \.self) { url in
                    ShareLink(item: url) { Text(url.lastPathComponent) }
                }
            }
            .navigationTitle("Swim files")
            .refreshable { receiver.refresh() }
        }
    }
}
