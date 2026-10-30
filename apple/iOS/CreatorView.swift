import SwimlieCore
import SwiftUI

/// Makes a swim or plan here with the coach, or builds an AI prompt from the form and the swimmer's history; an AI answer comes back through Paste in the Library.
struct CreatorView: View {
    @AppStorage("swimLevel") private var swimLevel = "intermediate"
    @ObservedObject var library: Library
    @Binding var path: [String] // generate pushes "preview" onto the Library's stack
    @AppStorage("allowCopyPaste") private var allowCopyPaste = false
    @AppStorage("aiEngine") private var aiEngine = "on-device"
    @AppStorage("aiEndpoint") private var aiEndpoint = "https://api.mistral.ai/v1"
    @AppStorage("aiProvider") private var aiProvider = "mistral"
    @AppStorage("aiModel") private var aiModel = ""
    private let gearOptions: [(Equipment, String)] = [(.fins, "Fins"), (.paddles, "Paddles"), (.pullBuoy, "Pull buoy"), (.kickboard, "Kickboard"), (.snorkel, "Snorkel")]
    private let kinds = ["easy recovery", "endurance", "threshold", "speed", "technique", "mixed"]

    @State private var step = 0 // 0 What, 1 How much, 2 Gear & strokes, 3 Anything else (Ask AI), 4 Review
    @FocusState private var askFocused: Bool
    @State private var poolLength: UInt32 = 25
    @State private var gear: Set<Equipment> = []
    @State private var startingBlocks = false
    @State private var byTime = true
    @State private var distance = 1500.0
    @State private var minutes = 45.0
    @State private var restPercent = 0.0
    @State private var strokes: Set<Stroke> = []
    private let strokeOptions: [(Stroke, String)] = [(.freestyle, "Freestyle"), (.backstroke, "Backstroke"), (.breaststroke, "Breaststroke"), (.butterfly, "Butterfly"), (.mixed, "IM")]
    /// The amount of a target, with " per swim" for a plan.
    private func amountText(time: Bool) -> String {
        if time { return isPlan ? String(localized: "\(Int(minutes)) min per swim") : String(localized: "\(Int(minutes)) min") }
        return isPlan ? String(localized: "\(Int(distance)) m per swim") : String(localized: "\(Int(distance)) m")
    }
    @AppStorage("creatorKind") private var kind = "endurance"
    @State private var notes = ""
    @State private var copied = false
    @State private var isPlan = false
    @State private var weeks = 6
    @State private var swimsPerWeek = 3
    @State private var request = ""
    @State private var aiBusy = false
    @State private var aiRecheck = 0
    @State private var aiReasonValue: String? // filled off the main thread by the .task below, never during render
    @State private var aiChecked = false // false until the first check lands; the button then reads Next
    @State private var aiError: String?
    #if DEBUG
    @State private var demoDone = false // the demo generates once, not again when coming back from the preview
    #endif

    var body: some View {
        // The Form is the root so the large title can bind to its scroll view straight away.
        Form {
            // Scrolls with the form (and its large title) instead of floating above it.
            Section {
                VStack(spacing: 4) {
                    HStack(spacing: 6) {
                        ForEach(0..<5) { i in
                            Capsule().fill(i <= step ? Color.accentColor : Color.secondary.opacity(0.3)).frame(height: 6)
                        }
                    }
                    Text("Step \(step + 1) of 5").font(.footnote).foregroundStyle(.secondary)
                }
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets(top: 0, leading: 0, bottom: 0, trailing: 0))
            }
            stepContent
        }
        .scrollDismissesKeyboard(.interactively)
        .safeAreaInset(edge: .bottom) {
            HStack(spacing: 12) {
                if step > 0 {
                    Button { step -= 1 } label: {
                        if step == 4 { Image(systemName: "chevron.left").font(.headline) } else { Label("Back", systemImage: "chevron.left").font(.headline) }
                    }
                    .accessibilityLabel("Back")
                        .modifier(NativeSecondary())
                }
                if step == 4 && allowCopyPaste {
                    Button { copyPrompt() } label: { Label(copied ? "Copied" : "Copy prompt", systemImage: "doc.on.doc").font(.headline) }
                        .modifier(NativeSecondary())
                }
                // Text in Ask AI means the AI fills the form (disabled when it is unavailable); an empty Ask AI reviews, with AI as an option.
                let aiReady = aiChecked && aiReason == nil
                let aiNeeded = step == 3 && !request.isEmpty
                Button {
                    if step == 4 { generate() } else if aiNeeded { askAI() } else { askFocused = false; step += 1 }
                } label: {
                    if aiBusy { ProgressView().padding(.horizontal, 8) }
                    else {
                        let title: LocalizedStringKey = step == 4 ? "Generate" : aiNeeded ? "Generate with AI" : "Next"
                        Label(title, systemImage: step == 4 ? "wand.and.stars" : aiNeeded ? "sparkles" : "chevron.right").font(.headline).lineLimit(1).minimumScaleFactor(0.7).padding(.horizontal, 8)
                    }
                }
                .disabled(aiBusy || (aiNeeded && !aiReady))
                .modifier(NativeProminent())
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 24)
        }
        .navigationTitle(isPlan ? "Create plan" : "Create swim")
        .task(id: "\(aiEngine)|\(aiEndpoint)|\(aiProvider)|\(aiModel)|\(aiRecheck)") {
            // Keychain and the on-device model are read off the main thread so the first frame is not blocked.
            let (engine, endpoint, provider, model) = (aiEngine, aiEndpoint, aiProvider, aiModel)
            aiReasonValue = await Task.detached { AIAssistant.reason(engine: engine, endpoint: endpoint, providerId: provider, model: model) }.value
            aiChecked = true
        }
        .task {
            // Re-check every few seconds while the on-device model downloads, so the button enables itself.
            guard #available(iOS 26, *) else { return }
            while await Task.detached(operation: { AIAssistant.isDownloading }).value {
                try? await Task.sleep(for: .seconds(3))
                aiRecheck += 1
            }
        }
        .onAppear {
            #if DEBUG
            if !demoDone, let n = demoArg?.hasPrefix("creator-") == true ? Int(demoArg!.dropFirst(8)) : nil { demoDone = true; step = n - 1 }
            if !demoDone, demoArg == "ai-generate" { demoDone = true; step = 3; request = "Easy 1500 with some kick, I'm a bit tired"; askAI() }
            if !demoDone, demoArg == "generate" || demoArg == "generate-plan" {
                demoDone = true
                isPlan = demoArg == "generate-plan"
                generate()
            }
            #endif
        }
    }

    @ViewBuilder private var stepContent: some View {
        switch step {
        case 0:
            Section {
                Picker("Mode", selection: $isPlan) {
                    Text("Single swim").tag(false)
                    Text("Plan").tag(true)
                }
                .pickerStyle(.segmented)
            }
            Section {
                Picker("Workout type", selection: $kind) {
                    ForEach(kinds, id: \.self) { Text(String(localized: String.LocalizationValue($0.capitalized))).tag($0) }
                }
            }
        case 1:
            Section {
                Picker("Pool length", selection: $poolLength) {
                    Text("25 m").tag(UInt32(25))
                    Text("50 m").tag(UInt32(50))
                }
                .pickerStyle(.segmented)
            }
            Section("Target") {
                Picker("Target", selection: $byTime) {
                    Text("Time").tag(true)
                    Text("Distance").tag(false)
                }
                .pickerStyle(.segmented)
                if byTime {
                    LabeledContent("Time", value: amountText(time: true))
                    Slider(value: $minutes, in: 15...120, step: 5)
                } else {
                    LabeledContent("Distance", value: amountText(time: false))
                    Slider(value: $distance, in: 500...5000, step: 100)
                }
                if !isPlan {
                    LabeledContent("Rest", value: restPercent == 0 ? String(localized: "Standard") : "\(restPercent > 0 ? "+" : "")\(Int(restPercent))%")
                    Slider(value: $restPercent, in: -50...50, step: 10)
                }
            }
            if isPlan {
                Section("Plan") {
                    Stepper("Weeks: \(weeks)", value: $weeks, in: 2...16)
                    Stepper("Swims per week: \(swimsPerWeek)", value: $swimsPerWeek, in: 1...6)
                }
            }
        case 2:
            Section("Equipment") {
                ForEach(gearOptions, id: \.1) { item, name in
                    Toggle(String(localized: String.LocalizationValue(name)), isOn: Binding(get: { gear.contains(item) }, set: { on in if on { gear.insert(item) } else { gear.remove(item) } }))
                }
                Toggle("Starting blocks", isOn: $startingBlocks)
            }
            if !isPlan {
                Section {
                    ForEach(strokeOptions, id: \.1) { item, name in
                        Toggle(String(localized: String.LocalizationValue(name)), isOn: Binding(get: { strokes.contains(item) }, set: { on in if on { strokes.insert(item) } else { strokes.remove(item) } }))
                    }
                } header: {
                    Text("Strokes")
                } footer: {
                    Text(strokes.isEmpty ? String(localized: "None selected: the coach picks the mix.") : String(localized: "The main sets are spread over these. IM needs sets of 4 lengths, butterfly stays short and easy."))
                }
            }
            Section {
                TextField("Anything else", text: $notes, axis: .vertical)
            } header: {
                Text("Notes")
            }
        case 3:
            Section {
                TextField("e.g. easy 1500 with kick, I'm tired", text: $request, axis: .vertical)
                    .focused($askFocused)
                    .submitLabel(.done)
                    .onSubmit { askFocused = false }
                    .onChange(of: request) { _, new in
                        // A vertical field turns Return into a newline; treat it as Done instead.
                        if new.contains("\n") {
                            request = new.replacingOccurrences(of: "\n", with: "")
                            askFocused = false
                        }
                    }
                if let reason = aiReason { Text(reason).font(.footnote).foregroundStyle(.secondary) }
                if let aiError { Text(aiError).font(.footnote).foregroundStyle(.red) }
            } header: {
                Text("Ask AI")
            } footer: {
                Text("Optional. The AI keeps your choices above unless you ask to change them.")
            }
        default:
            Section {
                LabeledContent("Mode", value: isPlan ? String(localized: "Plan") : String(localized: "Single swim"))
                LabeledContent("Workout type", value: String(localized: String.LocalizationValue(kind.capitalized)))
                if !request.isEmpty { LabeledContent("Ask AI", value: request) }
                LabeledContent("Pool length", value: poolLength == 50 ? "50 m" : "25 m")
                LabeledContent("Target", value: (byTime ? String(localized: "Time") : String(localized: "Distance")) + ": " + amountText(time: byTime))
                if !isPlan {
                    LabeledContent("Rest", value: restPercent == 0 ? String(localized: "Standard") : "\(restPercent > 0 ? "+" : "")\(Int(restPercent))%")
                } else {
                    LabeledContent("Weeks", value: "\(weeks)")
                    LabeledContent("Swims per week", value: "\(swimsPerWeek)")
                }
                LabeledContent("Equipment", value: gearOptions.filter { gear.contains($0.0) }.map { String(localized: String.LocalizationValue($0.1)) }.joined(separator: ", ").ifEmpty(String(localized: "None")))
                LabeledContent("Starting blocks", value: startingBlocks ? String(localized: "Yes") : String(localized: "No"))
                if !isPlan {
                    LabeledContent("Strokes", value: strokeOptions.filter { strokes.contains($0.0) }.map { String(localized: String.LocalizationValue($0.1)) }.joined(separator: ", ").ifEmpty(String(localized: "Coach picks the mix")))
                }
                LabeledContent("Notes", value: notes.ifEmpty(String(localized: "None")))
                if let aiError { Text(aiError).font(.footnote).foregroundStyle(.red) }
            } header: {
                Text("Review")
            } footer: {
                Text(allowCopyPaste ? String(localized: "Generate builds one here on the device and shows a preview to save. Copy prompt is for an AI app: paste it there, then use Paste in the Library to import the answer.") : String(localized: "Generate builds one here on the device and shows a preview to save."))
            }
            if request.isEmpty && aiChecked && aiReason == nil {
                Section {
                    Button { askAI() } label: {
                        HStack { Spacer(); if aiBusy { ProgressView() } else { Label("Generate with AI", systemImage: "sparkles") }; Spacer() }
                    }
                    .disabled(aiBusy)
                }
            }
        }
    }

    /// The form as the coach's input.
    private var inputs: CreatorInputs {
        CreatorInputs(poolLengthM: poolLength, equipment: gearOptions.map(\.0).filter { gear.contains($0) },
                      startingBlocks: startingBlocks,
                      targetDistanceM: byTime ? nil : UInt32(distance), targetTimeMin: byTime ? UInt32(minutes) : nil,
                      kind: kind, notes: notes, planWeeks: isPlan ? UInt32(weeks) : nil,
                      swimsPerWeek: isPlan ? UInt32(swimsPerWeek) : nil,
                      restPercent: isPlan ? 0 : Int32(restPercent),
                      strokes: isPlan ? [] : strokeOptions.map(\.0).filter { strokes.contains($0) },
                      language: Bundle.main.preferredLocalizations.first == "nl" ? "nl" : "en")
    }

    /// The swimmer's history, CSS and self-reported level (Settings), for the coach and the prompt.
    private var profile: SwimmerProfile {
        swimmerProfile(sessions: (try? Sync.store?.listSessions()) ?? [], cssSPer100m: try? Sync.store?.latestCssSPer100m(), nowMs: Int64(Date().timeIntervalSince1970 * 1000), level: swimmerLevel(stored: swimLevel))
    }

    /// Runs the coach on the device and shows the result; Save in GeneratedView puts it in the Library.
    private func generate() {
        let (inputs, profile) = (inputs, profile)
        library.previewNote = nil
        library.preview = isPlan ? .plan(plan: generatePlan(inputs: inputs, profile: profile)) : .workout(workout: generateWorkout(inputs: inputs, profile: profile))
        library.reshuffle = isPlan ? nil : { .workout(workout: generateWorkout(inputs: inputs, profile: profile)) }
        path.append("preview")
    }

    /// Why AI cannot be used, or nil when it can. The on-device model is checked only for the on-device engine.
    private var aiReason: String? { aiReasonValue }

    /// The AI prompt: `system` is the bundled instructions with the schema and example, `user` the form as a brief,
    /// the swimmer's Ask AI request (the AI keeps the form unless the request changes it) and the reply language.
    private var aiPromptParts: (system: String, user: String) {
        let (schema, example) = isPlan ? ("plan.schema", "plan-example") : ("workout.schema", "example")
        // The real schema and example replace the prompt's placeholder lines.
        let prompt = bundledText("prompt", "md").split(separator: "\n", omittingEmptySubsequences: false).filter { !["My request:", "Schema: <", "Example: <"].contains(where: $0.hasPrefix) }.joined(separator: "\n")
        let system = [prompt, "Schema:", bundledText(schema, "json"), "Example:", bundledText(example, "json")].joined(separator: "\n")
        var user = [creatorBrief(inputs: inputs, profile: profile)]
        let ask = request.trimmingCharacters(in: .whitespacesAndNewlines)
        if !ask.isEmpty { user.append("My request: \(ask)") }
        if let code = Bundle.main.preferredLocalizations.first, code != "en",
           let language = Locale(identifier: "en").localizedString(forLanguageCode: code) {
            user.append("Write the workout name, notes and description in \(language); keep JSON keys, enum values and drill ids in English.")
        }
        return (system, user.joined(separator: "\n"))
    }

    /// The AI writes the whole workout or plan; it is previewed like a generated one, and Save in GeneratedView keeps it.
    private func askAI() {
        aiBusy = true
        aiError = nil
        let parts = aiPromptParts
        Task {
            do {
                let reply = try await AIAssistant.generate(system: parts.system, user: parts.user)
                let imported = fillMissingRest(parsed: try parseImport(json: AIAssistant.jsonObject(in: reply)))
                try checkEquipment(import: imported, allowed: inputs.equipment)
                aiBusy = false
                library.previewNote = nil
                library.preview = imported
                library.reshuffle = nil
                path.append("preview")
            } catch {
                aiBusy = false
                let detail = error.localizedDescription.split(separator: "\n").first.map(String.init) ?? ""
                aiError = String(localized: "The AI did not return a usable workout: \(detail)")
            }
        }
    }

    private func copyPrompt() {
        let parts = aiPromptParts
        UIPasteboard.general.string = parts.system + "\n" + parts.user
        copied = true
    }
}

private extension String {
    /// This text, or the fallback when it is empty.
    func ifEmpty(_ fallback: String) -> String { isEmpty ? fallback : self }
}
