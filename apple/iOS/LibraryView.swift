import SwimlieCore
import SwiftUI
import UniformTypeIdentifiers

/// Stored workouts and plans plus the import and delete actions, shared by LibraryView and the share-sheet open.
@MainActor final class Library: ObservableObject {
    @Published var workouts: [StoredWorkout] = []
    @Published var plans: [StoredPlan] = []
    @Published var css: Double? // latest CSS pace, for the duration estimate
    @Published var swum: Set<String> = [] // workout ids of the swims done, from the sessions
    @Published var error: String?
    @Published var preview: Import? // generated in the creator, until saved
    var reshuffle: (() -> Import)? // makes another workout from the same inputs
    var previewNote: String? // the AI coach note for a generated plan (a workout keeps it in its description)

    func reload() {
        do {
            workouts = try Sync.store?.listWorkouts() ?? []
            plans = try Sync.store?.listPlans() ?? []
            css = try Sync.store?.latestCssSPer100m()
            let sessions = try Sync.store?.listSessions() ?? []
            swum = Set(sessions.compactMap { $0.session.workoutId })
        } catch { self.error = error.localizedDescription }
    }

    func save(_ item: Import, sendToWatch: Bool = false) {
        do {
            switch item {
            case .workout(let workout):
                if let saved = try Sync.store?.saveWorkout(id: nil, workout: workout), sendToWatch { try Sync.sendWorkout(saved) }
            case .plan(let plan): _ = try Sync.store?.savePlan(id: nil, plan: plan)
            }
            Sync.send()
            reload()
        } catch { self.error = error.localizedDescription }
    }

    func importJSON(_ text: String) {
        do { save(try parseImport(json: text)) } catch WorkoutError.Json(let msg) {
            self.error = "Not valid JSON: \(msg)"
        } catch WorkoutError.Invalid(let msg) {
            self.error = "Not a valid workout or plan: \(msg)"
        } catch { self.error = error.localizedDescription }
    }

    func importFile(_ url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        do { importJSON(try String(contentsOf: url, encoding: .utf8)) } catch { self.error = error.localizedDescription }
    }

    func delete(_ id: String) {
        do {
            _ = try Sync.store?.deleteWorkout(id: id)
            Sync.send()
            reload()
        } catch { self.error = error.localizedDescription }
    }

    func deletePlan(_ id: String) {
        do {
            _ = try Sync.store?.deletePlan(id: id)
            Sync.send()
            reload()
        } catch { self.error = error.localizedDescription }
    }
}

/// One workout in a list: badge, name, distance, estimated minutes; the watch icon and done check come first.
struct WorkoutRow: View {
    let workout: Workout
    let css: Double?
    let onWatch: Bool
    let done: Bool
    var send: (() -> Void)? = nil

    var body: some View {
        let minutes = Int((estimatedDurationS(workout: workout, cssSPer100m: css) / 60).rounded())
        HStack(spacing: 12) {
            IconBadge(systemName: "figure.pool.swim", metric: .distance)
            VStack(alignment: .leading, spacing: 2) {
                Text(workout.name).font(.headline)
                HStack(alignment: .firstTextBaseline, spacing: 3) {
                    Text("\(totalDistanceM(workout: workout))").font(.system(.title2, design: .rounded).weight(.semibold))
                    Text("M").font(.system(.title3, design: .rounded).weight(.semibold))
                }
                .foregroundStyle(Metric.distance.color)
            }
            Spacer()
            if let send {
                Button(action: send) { Image(systemName: "applewatch").font(.title2).foregroundStyle(onWatch ? Color.white : Color.blue)
                    .frame(width: 48, height: 48).background(onWatch ? AnyShapeStyle(Color.blue) : AnyShapeStyle(.quaternary), in: Circle()) }
                    .buttonStyle(.borderless)
                    .accessibilityLabel(onWatch ? "Sent to watch" : "Send to watch")
            }
            if done { Image(systemName: "checkmark.circle.fill").foregroundStyle(.green) }
            VStack(alignment: .trailing, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 3) {
                    Text("\(minutes)").font(.system(.title3, design: .rounded).weight(.semibold))
                    Text("min").font(.system(.body, design: .rounded).weight(.semibold))
                }
                .foregroundStyle(Metric.time.color)
                Text("estimated").font(.footnote).foregroundStyle(.secondary)
            }
        }
    }
}

struct LibraryView: View {
    @ObservedObject var library: Library
    @AppStorage("allowCopyPaste") private var allowCopyPaste = false
    @State private var choosingFile = false
    @State private var onWatch: String? // id of the workout last sent to the watch
    @State private var path: [String] = demoCreator ? ["creator"] : [] // "creator" pushes CreatorView, "plan:<id>" a PlanView, "preview" the generated one

    private func send(_ w: StoredWorkout) {
        do { try Sync.sendWorkout(w); onWatch = w.id } catch { library.error = error.localizedDescription }
    }

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    HStack(spacing: 12) {
                        IconBadge(systemName: "stopwatch", metric: .time)
                        Text("CSS pace").font(.headline)
                        Spacer()
                        if let css = library.css {
                            Text("\(clock(css)) /100 m").font(.system(.title3, design: .rounded).weight(.semibold)).foregroundStyle(Metric.time.color)
                        } else {
                            Text("Not set: swim the CSS test").font(.subheadline).foregroundStyle(.secondary)
                        }
                    }
                } header: {
                    Text("CSS")
                } footer: {
                    Text(String(localized: "Updated automatically after a CSS test swim.") + (BuildInfo.label.map { "\n\($0)" } ?? ""))
                }
                if !library.plans.isEmpty {
                    Section("Plans") {
                        ForEach(library.plans, id: \.id) { p in
                            let ids = planWorkoutIds(p)
                            let done = ids.filter { library.swum.contains($0) }.count
                            NavigationLink(value: "plan:\(p.id)") {
                                HStack(spacing: 12) {
                                    IconBadge(systemName: "calendar", metric: .distance)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(p.plan.name).font(.headline)
                                        Text(p.plan.weeks.count == 1 ? String(localized: "1 week · \(done)/\(ids.count) swims") : String(localized: "\(p.plan.weeks.count) weeks · \(done)/\(ids.count) swims"))
                                            .font(.subheadline).foregroundStyle(.secondary)
                                    }
                                }
                            }
                            .swipeActions {
                                Button(role: .destructive) { library.deletePlan(p.id) } label: { Label("Delete", systemImage: "trash") }
                            }
                        }
                    }
                }
                if !library.workouts.isEmpty {
                    Section("Workouts") {
                        ForEach(library.workouts, id: \.id) { w in
                            NavigationLink(value: "workout:\(w.id)") {
                                WorkoutRow(workout: w.workout, css: library.css, onWatch: onWatch == w.id, done: false, send: { send(w) })
                            }
                                .swipeActions {
                                    if w.id != Sync.cssTestId {
                                        Button(role: .destructive) { library.delete(w.id) } label: { Label("Delete", systemImage: "trash") }
                                    }
                                }
                        }
                    }
                }
            }
            .overlay {
                if library.workouts.isEmpty && library.plans.isEmpty {
                    ContentUnavailableView("No workouts yet", systemImage: "list.bullet.clipboard",
                                           description: Text("Import a workout or plan: paste JSON, open a .json file or share one to Swimlie."))
                }
            }
            .navigationTitle("Workouts")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu("Import", systemImage: "square.and.arrow.down") {
                        if allowCopyPaste {
                            Button("Paste") { library.importJSON(UIPasteboard.general.string ?? "") }
                        }
                        Button("Choose file") { choosingFile = true }
                        if allowCopyPaste {
                            Button("Copy AI prompt") { UIPasteboard.general.string = bundledText("prompt", "md") + "\n" + bundledText("workout.schema", "json") }
                        }
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    NavigationLink(value: "creator") { Label("Create swim", systemImage: "wand.and.stars") }
                }
            }
            .navigationDestination(for: String.self) { value in
                if value == "creator" {
                    CreatorView(library: library, path: $path)
                } else if value == "preview", let item = library.preview {
                    GeneratedView(item: item, library: library) { path = [] }
                } else if let p = library.plans.first(where: { "plan:\($0.id)" == value }) {
                    PlanView(plan: p, library: library)
                } else if let w = library.workouts.first(where: { "workout:\($0.id)" == value }) {
                    WorkoutDetail(workout: w.workout, css: library.css)
                }
            }
            .fileImporter(isPresented: $choosingFile, allowedContentTypes: [.json]) { result in
                switch result {
                case .success(let url): library.importFile(url)
                case .failure(let error): library.error = error.localizedDescription
                }
            }
            .refreshable { library.reload() }
            .onAppear {
                library.reload(); onWatch = Sync.sentWorkoutId
                #if DEBUG
                if demoArg == "plans" || demoArg == "plan", library.plans.isEmpty { library.importJSON(bundledText("plan-example", "json")) }
                if demoArg == "plan", let p = library.plans.first { path = ["plan:\(p.id)"] }
                if let kind = demoDrills, library.preview == nil { // the preview is WorkoutDetail with a Save button
                    let inputs = CreatorInputs(poolLengthM: 25, equipment: [.pullBuoy], startingBlocks: true, targetDistanceM: 2000, targetTimeMin: nil,
                                               kind: kind, notes: "", planWeeks: nil, swimsPerWeek: nil, restPercent: 0, strokes: demoArg == "rounds" ? [.freestyle, .breaststroke, .backstroke] : [], language: "en")
                    library.preview = .workout(workout: generateWorkout(inputs: inputs, profile: swimmerProfile(sessions: [], cssSPer100m: nil, nowMs: Int64(Date().timeIntervalSince1970 * 1000), level: swimmerLevel(stored: UserDefaults.standard.string(forKey: "swimLevel") ?? ""))))
                    path = ["preview"]
                }
                #endif
            }
            .onReceive(NotificationCenter.default.publisher(for: Sync.changed)) { _ in library.reload() }
            .errorAlert($library.error)
        }
    }
}

func bundledText(_ name: String, _ ext: String) -> String {
    Bundle.main.url(forResource: name, withExtension: ext).flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? ""
}

#if DEBUG
/// `-demo creator` opens the creator screen, `-demo creator-3` at its step 3; `-demo plans` imports the example plan; `-demo plan` also opens it.
/// `-demo generate` opens the creator and generates a swim with its defaults (endurance, 1500 m, 25 m pool), `-demo generate-plan` a plan (6 weeks, 3 swims). For screenshots.
let demoArg = UserDefaults.standard.string(forKey: "demo")
private let demoCreator = demoArg?.hasPrefix("creator") == true || ["generate", "generate-plan", "ai-generate"].contains(demoArg)
/// `-demo drills` previews a technique swim with starting blocks, `-demo drills-speed` a speed one, for the drill lines.
private let demoDrills = demoArg == "drills" ? "technique" : demoArg == "drills-speed" ? "speed" : demoArg == "rounds" ? "endurance" : nil
#else
private let demoCreator = false
#endif
