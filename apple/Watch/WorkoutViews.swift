import CoreLocation
import SwimlieCore
import SwiftUI

/// Sent workout details (start) -> live -> summary, driven by the session's own state.
struct RootView: View {
    let store: Store
    let session: any SwimSessionModel
    @AppStorage("sentWorkoutId") private var sentId = "" // written by Sync.receiveWorkout
    @AppStorage("sentWorkoutJSON") private var sentJSON = ""
    @State private var swimming = false // Done on the summary returns to the picker
    @AppStorage("saveLocation") private var saveLocation = false
    @AppStorage("waterLock") private var waterLock = true
    @State private var asker = CLLocationManager() // kept alive while the permission prompt is up
    let recorderView: AnyView

    private var sent: StoredWorkout? {
        guard let w = try? parseWorkout(json: sentJSON) else { return nil }
        return StoredWorkout(id: sentId, createdAtMs: 0, updatedAtMs: 0, workout: w)
    }

    var body: some View {
        NavigationStack {
            if swimming {
                if session.ended {
                    SummaryView(saved: session.saved, workout: session.workout, error: session.error) { swimming = false }
                } else {
                    LiveView(session: session, css: try? store.latestCssSPer100m()) { session.end() }
                }
            } else if let w = sent {
                // The sent workout's details are the start screen; Recorder and settings sit behind the toolbar button.
                DetailView(store: store, workout: w.workout) {
                    session.start(stored: w)
                    swimming = true
                }
                .toolbar { ToolbarItem(placement: .topBarTrailing) { NavigationLink { menu } label: { Image(systemName: "ellipsis") } } }
            } else {
                menu
            }
        }
        .themed()
        .onChange(of: session.workout != nil) { if session.workout != nil, !session.ended { swimming = true } } // a recovered swim
        .task {
            if session.workout != nil { swimming = true } // started before the view appeared (recovery, demo)
        }
    }
    private var menu: some View {
        ScrollViewReader { proxy in
            List {
                if sent == nil { Text("Send a workout from the Swimlie app on your iPhone.").font(.footnote).foregroundStyle(.secondary) }
                NavigationLink("Recorder") { recorderView }
                if let e = session.error { Text(e).font(.footnote).foregroundStyle(.red) }
                Section {
                    Toggle("Save swim location", isOn: $saveLocation)
                        .onChange(of: saveLocation) { if saveLocation { asker.requestWhenInUseAuthorization() } }
                } footer: {
                    Text("One position at the start, to name the pool. Stays on your devices.")
                }
                Section {
                    Toggle("Water lock", isOn: $waterLock)
                } footer: {
                    Text("Locks the touchscreen while swimming. Turn off for testing.")
                }
                .id("location")
                if let build = BuildInfo.label { Text(build).font(.footnote).foregroundStyle(.secondary) }
            }
            #if DEBUG
            .onAppear { if UserDefaults.standard.string(forKey: "demo") == "picker-bottom" { proxy.scrollTo("location") } } // screenshot of the scrolled picker
            #endif
            .navigationTitle("Swim")
        }
    }
}

#if DEBUG
@MainActor private func previewSession() -> FakeSwimSession {
    let session = FakeSwimSession(store: try! Store.open(path: ":memory:"))
    let w = try! parseWorkout(json: #"{"version":1,"name":"P","pool_length_m":25,"blocks":[{"name":"Main","sets":[{"reps":4,"distance_m":50,"rest":{"type":"fixed","seconds":20}}]}]}"#)
    session.start(stored: StoredWorkout(id: "preview", createdAtMs: 0, updatedAtMs: 0, workout: w))
    return session
}

#Preview("Live") { LiveView(session: previewSession(), css: nil) {} }

@MainActor private func previewSaved() -> FakeSwimSession {
    let session = previewSession()
    session.fastForward(toS: 400)
    session.end()
    return session
}

#Preview("Summary") { SummaryView(saved: previewSaved().saved, workout: previewSession().workout) {} }
#endif
