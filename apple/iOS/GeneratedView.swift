import SwimlieCore
import SwiftUI

/// A generated workout or plan before it is saved; Save puts it in the Library and goes back there.
struct GeneratedView: View {
    let item: Import
    @ObservedObject var library: Library
    let onSave: () -> Void

    var body: some View {
        Group {
            switch item {
            case .workout(let workout): WorkoutDetail(workout: workout, css: library.css)
            case .plan(let plan): PlanDetail(plan: plan, css: library.css, note: library.previewNote)
            }
        }
        .safeAreaInset(edge: .bottom) {
            if case .workout = item {
                Button { library.save(item, sendToWatch: true); onSave() } label: {
                    Label("Send to watch", systemImage: "applewatch").font(.headline).padding(.horizontal, 12)
                }
                .modifier(NativeProminent())
                .padding(.bottom, 24)
            }
        }
        .toolbar {
            if let reshuffle = library.reshuffle {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { library.preview = reshuffle() } label: { Label("Shuffle", systemImage: "shuffle") }
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button("Save") { library.save(item); onSave() }
            }
        }
    }
}

/// The row on top, then one section per block with a line per set.
struct WorkoutDetail: View {
    let workout: Workout
    let css: Double?

    var body: some View {
        List {
            Section {
                WorkoutRow(workout: workout, css: css, onWatch: false, done: false)
            }
            if let note = workout.description { CoachNoteSection(note: note) }
            let gear = workout.blocks.flatMap(\.sets).flatMap(\.equipment).reduce(into: [Equipment]()) { if !$0.contains($1) { $0.append($1) } }
            if !gear.isEmpty {
                Section("Equipment") {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 120), alignment: .leading)], alignment: .leading, spacing: 8) {
                        ForEach(gear, id: \.self) { GearLabel(equipment: $0).font(.subheadline) }
                    }
                }
            }
            ForEach(workout.blocks.indices, id: \.self) { b in
                let block = workout.blocks[b]
                let repeated = block.repeat > 1
                Section {
                    ForEach(block.sets.indices, id: \.self) { i in
                        SetLine(set: block.sets[i])
                            // A repeated block: an accent bar along its sets reads as one round.
                            .padding(.leading, repeated ? 10 : 0)
                            .overlay(alignment: .leading) { if repeated { Capsule().fill(Color.accentColor).frame(width: 4) } }
                    }
                } header: {
                    HStack {
                        Text(block.name)
                        Spacer()
                        if repeated {
                            Label("Repeat \(block.repeat)×", systemImage: "repeat").font(.footnote.bold()).foregroundStyle(Color.accentColor)
                                .padding(.horizontal, 10).padding(.vertical, 3).background(Color.accentColor.opacity(0.18), in: Capsule())
                        }
                    }
                } footer: {
                    if repeated { Text("\(block.repeat) rounds of \(block.sets.reduce(0) { $0 + Int($1.reps * $1.distanceM) }) m") }
                }
            }
        }
        .navigationTitle(workout.name)
    }
}

/// The weeks as sections like PlanView; a workout opens its own preview.
private struct PlanDetail: View {
    let plan: Plan
    let css: Double?
    var note: String? = nil

    var body: some View {
        List {
            if let note { CoachNoteSection(note: note) }
            ForEach(plan.weeks.indices, id: \.self) { w in
                let week = plan.weeks[w]
                Section {
                    ForEach(week.workouts.indices, id: \.self) { i in
                        let workout = week.workouts[i]
                        NavigationLink {
                            WorkoutDetail(workout: workout, css: css)
                        } label: {
                            WorkoutRow(workout: workout, css: css, onWatch: false, done: false)
                        }
                    }
                } header: {
                    Text(week.focus.map { String(localized: "Week \(w + 1) · \($0)") } ?? String(localized: "Week \(w + 1)"))
                }
            }
        }
        .navigationTitle(plan.name)
    }
}

/// "4 × 100 m Freestyle", then intensity, goal, rest, equipment and notes on a second line; a drill adds its how-to under them.
struct SetLine: View {
    let set: WorkoutSet

    var body: some View {
        let e = set.drill.flatMap { exerciseIn(id: $0, lang: Bundle.main.preferredLocalizations.first == "nl" ? "nl" : "en") }
        let rest = set.rest.map { $0.kind == .interval ? String(localized: "every \(clock(Double($0.seconds)))") : String(localized: "rest \(clock(Double($0.seconds)))") }
        let parts: [String?] = [intensityName(set.intensity), set.goalS.map { String(localized: "goal \(clock(Double($0)))") }, rest,
                                set.notes == e?.cue ? nil : set.notes] // the cue is the drill's own line below
        VStack(alignment: .leading, spacing: 2) {
            // A drill set is stored as Any: show the drill's own stroke (Any again only if the drill has none).
            let stroke = set.stroke == .any && e != nil ? (e!.stroke == .any ? String(localized: "Drill") : e!.stroke.name) : set.stroke.name
            Text("\(set.reps) × \(set.distanceM) m \(stroke)").font(.headline)
            Text(parts.compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · "))
                .font(.subheadline).foregroundStyle(.secondary)
            if !set.equipment.isEmpty {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 100), alignment: .leading)], alignment: .leading, spacing: 4) {
                    ForEach(set.equipment, id: \.self) { GearLabel(equipment: $0).font(.footnote) }
                }
                .foregroundStyle(.blue)
            }
            if let e {
                Text(e.name).font(.subheadline.weight(.semibold)).padding(.top, 4)
                Text(e.howTo).font(.footnote).foregroundStyle(.secondary)
                if let safety = e.safety {
                    Label(safety, systemImage: "exclamationmark.triangle").font(.footnote).foregroundStyle(.orange)
                }
                if let url = URL(string: e.videoUrl) {
                    Link(destination: url) { Label("Watch how", systemImage: "play.rectangle").font(.footnote) }
                        .accessibilityLabel("Watch how to do \(e.name) on YouTube")
                }
            }
        }
    }
}

private func intensityName(_ intensity: Intensity) -> String {
    switch intensity {
    case .easy: String(localized: "Easy")
    case .moderate: String(localized: "Moderate")
    case .threshold: String(localized: "Threshold")
    case .hard: String(localized: "Hard")
    case .sprint: String(localized: "Sprint")
    }
}

/// The same template images as the watch (asset catalog), so they match there.
func equipmentIcon(_ equipment: Equipment) -> String {
    switch equipment {
    case .fins: "fins"
    case .paddles: "paddles"
    case .pullBuoy: "pull_buoy"
    case .kickboard: "kickboard"
    case .snorkel: "snorkel"
    }
}

/// An equipment label with its watch icon at text height.
struct GearLabel: View {
    let equipment: Equipment

    var body: some View {
        Label {
            Text(equipmentName(equipment))
        } icon: {
            Image(equipmentIcon(equipment)).resizable().scaledToFit().frame(width: 18, height: 18)
        }
    }
}

func equipmentName(_ equipment: Equipment) -> String {
    switch equipment {
    case .fins: String(localized: "Fins")
    case .paddles: String(localized: "Paddles")
    case .pullBuoy: String(localized: "Pull buoy")
    case .kickboard: String(localized: "Kickboard")
    case .snorkel: String(localized: "Snorkel")
    }
}

/// The AI coach note: a compact card with a sparkles icon, shown above the swim or plan.
struct CoachNoteSection: View {
    let note: String

    var body: some View {
        Section {
            VStack(alignment: .leading, spacing: 6) {
                Label("Coach's note", systemImage: "sparkles").font(.subheadline.bold()).foregroundStyle(Color.accentColor)
                Text(note).font(.subheadline).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }
}

/// Liquid Glass on iOS 26+, a prominent capsule button before that.
struct NativeProminent: ViewModifier {
    func body(content: Content) -> some View {
        if #available(iOS 26, *) { content.buttonStyle(.glassProminent).controlSize(.large) }
        else { content.buttonStyle(.borderedProminent).buttonBorderShape(.capsule).controlSize(.large) }
    }
}

/// The quieter partner of NativeProminent: Liquid Glass on iOS 26+, a bordered capsule before that.
struct NativeSecondary: ViewModifier {
    func body(content: Content) -> some View {
        if #available(iOS 26, *) { content.buttonStyle(.glass).controlSize(.large) }
        else { content.buttonStyle(.bordered).buttonBorderShape(.capsule).controlSize(.large) }
    }
}
