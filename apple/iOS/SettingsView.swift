import SwimlieCore
import SwiftUI

/// The swimmer level the Settings picker stores as "beginner", "intermediate" or "advanced" (PLAN.md 4b).
func swimmerLevel(stored: String) -> SwimLevel {
    switch stored {
    case "beginner": .beginner
    case "advanced": .advanced
    default: .intermediate
    }
}

struct SettingsView: View {
    @AppStorage("allowCopyPaste") private var allowCopyPaste = false
    @AppStorage("debug") private var debug = false
    @AppStorage("aiEngine") private var aiEngine = "on-device"
    @AppStorage("aiProvider") private var providerId = "mistral"
    @AppStorage("aiEndpoint") private var aiEndpoint = "https://api.mistral.ai/v1"
    @AppStorage("aiModel") private var aiModel = ""
    @AppStorage("aiReviewPrompt") private var reviewPrompt = ""
    @AppStorage("swimLevel") private var swimLevel = "intermediate"
    @State private var apiKey = ""
    @State private var showOtherModel = false
    @State private var loadingModels = false
    @State private var modelsError: String?
    /// Models fetched per provider, so switching back is instant.
    @State private var modelCache: [String: [String]] = [:]
    /// Set when the provider changed and the model list has not been applied yet.
    @State private var providerChanged = false
    @State private var keyTask: Task<Void, Never>?

    private let otherTag = "__other__"

    private var provider: AIProvider { AIProvider.find(providerId) }
    private var modelOptions: [String] {
        let fetched = modelCache[providerId] ?? []
        return fetched.isEmpty ? provider.models : fetched
    }

    /// The Picker shows the models; "Other…" (or an empty list, as for Custom) reveals the text field.
    private var modelSelection: Binding<String> {
        Binding(
            get: { showOtherModel ? otherTag : aiModel },
            set: { new in
                showOtherModel = new == otherTag
                if !showOtherModel { aiModel = new }
            })
    }

    var body: some View {
        Form {
            Section {
                Toggle("Copy/paste AI flow", isOn: $allowCopyPaste)
            } footer: {
                Text("Copy a prompt to a chat app and paste the reply back into Swimlie.")
            }
            Section {
                Picker("Swimmer level", selection: $swimLevel) {
                    Text("Just learning").tag("beginner")
                    Text("Can swim 200 m non-stop").tag("intermediate")
                    Text("Regular swimmer").tag("advanced")
                }
            } footer: {
                Text("Used for the paces of workouts when there is no CSS test.")
            }
            Section {
                Toggle("Debug", isOn: $debug)
            } footer: {
                Text("Shows the raw swim files.")
            }
            Section {
                Picker("Engine", selection: $aiEngine) {
                    Text("On-device").tag("on-device")
                    Text("API").tag("api")
                }
                if aiEngine == "api" {
                    Picker("Provider", selection: $providerId) {
                        ForEach(AIProvider.all) { Text($0.name).tag($0.id) }
                    }
                    if providerId == "custom" {
                        TextField("Endpoint", text: $aiEndpoint, prompt: Text("https://api.example.com/v1"))
                            .keyboardType(.URL)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                    }
                    if !modelOptions.isEmpty {
                        Picker(selection: modelSelection) {
                            ForEach(modelOptions, id: \.self) { Text($0).tag($0) }
                            Text("Other…").tag(otherTag)
                        } label: {
                            HStack(spacing: 6) {
                                Text("Model")
                                if loadingModels { ProgressView().controlSize(.small) }
                            }
                        }
                    }
                    if showOtherModel || modelOptions.isEmpty {
                        TextField("Model name", text: $aiModel, prompt: Text("e.g. mistral-small-latest"))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                    }
                    SecureField("API key", text: $apiKey)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onChange(of: apiKey) { _, new in
                            KeychainStore.write(AIProvider.keyAccount(providerId), new.trimmingCharacters(in: .whitespacesAndNewlines))
                            // Debounced: load the list once typing (or pasting) pauses.
                            keyTask?.cancel()
                            guard !new.trimmingCharacters(in: .whitespaces).isEmpty else { return }
                            keyTask = Task {
                                try? await Task.sleep(for: .milliseconds(800))
                                guard !Task.isCancelled else { return }
                                await loadModels()
                            }
                        }
                    if let modelsError {
                        Text(modelsError).font(.footnote).foregroundStyle(.secondary)
                    }
                }
                NavigationLink {
                    ReviewPromptEditor(prompt: $reviewPrompt)
                } label: {
                    LabeledContent("Review prompt", value: reviewPrompt.isEmpty ? String(localized: "Default") : String(localized: "Custom"))
                }
            } header: {
                Text("AI")
            } footer: {
                Text(aiEngine == "api"
                     ? "The key is stored in the iOS Keychain and sent only to the chosen provider."
                     : "The on-device model runs on this iPhone. Pick API to use a remote model instead.")
            }
            Section {
                LabeledContent("Version", value: BuildInfo.version)
                LabeledContent("Commit", value: BuildInfo.commit ?? String(localized: "unknown"))
            } header: {
                Text("About")
            } footer: {
                Text("Swimlie keeps your swims on your devices. A swim summary leaves the phone only when you ask the AI to review it, and goes to the provider you chose.")
            }
        }
        .navigationTitle("Settings")
        .scrollDismissesKeyboard(.interactively)
        .task(id: "\(aiEngine)|\(providerId)|\(aiEndpoint)") { await loadModels() }
        .onAppear {
            // One-time move of the key saved before providers existed (it was for the Mistral default).
            if let legacy = KeychainStore.read("aiApiKey") {
                KeychainStore.write(AIProvider.keyAccount("mistral"), legacy)
                KeychainStore.delete("aiApiKey")
            }
            // A stored provider id the app no longer knows shows as the first provider; save what is shown.
            if providerId != provider.id { providerId = provider.id }
            if aiEndpoint.isEmpty, !provider.endpoint.isEmpty { aiEndpoint = provider.endpoint }
            apiKey = KeychainStore.read(AIProvider.keyAccount(providerId)) ?? ""
            if aiModel.isEmpty { aiModel = provider.models.first ?? "" }
            showOtherModel = !modelOptions.contains(aiModel)
        }
        .onChange(of: providerId) { _, new in
            let p = AIProvider.find(new)
            if !p.endpoint.isEmpty { aiEndpoint = p.endpoint }
            providerChanged = true
            modelsError = nil
            apiKey = KeychainStore.read(AIProvider.keyAccount(new)) ?? ""
            reconcileModel()
        }
    }

    /// Asks the provider for its model list (off the main thread), caches it, and falls back silently to the built-in list.
    private func loadModels() async {
        guard aiEngine == "api", !aiEndpoint.isEmpty else {
            providerChanged = false
            return
        }
        let p = providerId
        let endpoint = aiEndpoint
        let key = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        loadingModels = true
        do {
            let list = try await Task.detached { try aiModels(endpoint: endpoint, apiKey: key) }.value
            if p == providerId { modelCache[p] = list; modelsError = nil }
        } catch {
            // Without a key the public list simply isn't available; only mention failures when a key is set.
            if p == providerId { modelsError = key.isEmpty ? nil : "Models unavailable: \(error.localizedDescription)" }
        }
        loadingModels = false
        if p == providerId { reconcileModel() }
    }

    /// Keeps the selected model when the list has it; after a provider switch picks the first listed model instead.
    private func reconcileModel() {
        let switched = providerChanged
        providerChanged = false
        let options = modelOptions
        if options.contains(aiModel) {
            showOtherModel = false
        } else if switched {
            aiModel = options.first ?? ""
            showOtherModel = false
        } else {
            showOtherModel = true
        }
    }
}

/// Full-screen editor for the AI review prompt; an empty stored value means the built-in prompt.
private struct ReviewPromptEditor: View {
    @Binding var prompt: String
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            TextEditor(text: Binding(get: { prompt.isEmpty ? defaultReviewPrompt() : prompt },
                                     set: { prompt = $0 == defaultReviewPrompt() ? "" : $0 }))
                .focused($focused)
                .font(.body)
                .scrollContentBackground(.hidden)
                .padding(12)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 16))
            Text("Sent as the instruction with every review, to the on-device model or your provider. Keep the five section labels (Summary, Achievements, Progress, What to improve, Next time) to see them in bold.")
                .font(.footnote).foregroundStyle(.secondary)
            if !prompt.isEmpty {
                Button("Reset to default", role: .destructive) { prompt = "" }
            }
        }
        .padding()
        .navigationTitle("Review prompt")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if focused {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { focused = false } }
            }
        }
        .scrollDismissesKeyboard(.interactively)
    }
}
