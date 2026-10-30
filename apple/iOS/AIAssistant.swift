import Foundation
import FoundationModels
import SwimlieCore

enum AIAssistantError: LocalizedError {
    case unavailable(String)
    var errorDescription: String? { if case .unavailable(let reason) = self { return reason }; return nil }
}

/// Forwards each streamed chunk of the remote reply to the main actor.
final class PartialSink: ReviewSink, @unchecked Sendable {
    private let onPartial: @MainActor (String) -> Void
    init(_ onPartial: @escaping @MainActor (String) -> Void) { self.onPartial = onPartial }
    func onText(textSoFar: String) {
        Task { @MainActor in self.onPartial(textSoFar) }
    }
}

/// Wraps the on-device Foundation Models session and the remote API. Rust parses and checks the workout the AI writes.
enum AIAssistant {
    @available(iOS 26, *)
    static var isDownloading: Bool { SystemLanguageModel.default.availability == .unavailable(.modelNotReady) }

    @available(iOS 26, *)
    /// nil when the model can be used, otherwise a one-line reason to show.
    static var unavailableReason: String? {
        switch SystemLanguageModel.default.availability {
        case .available: return nil
        case .unavailable(.deviceNotEligible): return String(localized: "This device does not support Apple Intelligence (iPhone 15 Pro or newer is needed)")
        case .unavailable(.appleIntelligenceNotEnabled): return String(localized: "Apple Intelligence is off. Turn it on in Settings › Apple Intelligence & Siri")
        case .unavailable(.modelNotReady): return String(localized: "The on-device model is downloading or starting up. This needs Wi-Fi and some free space; try again in a few minutes. Status: Settings › Apple Intelligence & Siri")
        case .unavailable(_): return String(localized: "On-device AI is unavailable right now. Try again later")
        }
    }

    /// nil when the chosen engine can be used, otherwise a one-line reason to show.
    static func reason(engine: String, endpoint: String, providerId: String, model: String) -> String? {
        if engine == "api" {
            let key = KeychainStore.read(AIProvider.keyAccount(providerId)) ?? ""
            return endpoint.trimmingCharacters(in: .whitespaces).isEmpty || key.isEmpty || model.trimmingCharacters(in: .whitespaces).isEmpty
                ? String(localized: "Add an endpoint, model and API key in Settings") : nil
        }
        if #available(iOS 26, *) { return unavailableReason }
        return String(localized: "Needs iOS 26 or later")
    }

    /// The endpoint, key and model of the chosen API provider (Settings), with the provider's preset as fallback.
    fileprivate static func remoteSettings() -> (endpoint: String, key: String, model: String) {
        let d = UserDefaults.standard
        let providerId = d.string(forKey: "aiProvider") ?? "mistral"
        // @AppStorage defaults are not written to UserDefaults, so fall back to the provider's preset.
        let preset = AIProvider.find(providerId)
        let stored = d.string(forKey: "aiEndpoint") ?? ""
        let endpoint = stored.isEmpty ? preset.endpoint : stored
        let storedModel = d.string(forKey: "aiModel") ?? ""
        let model = storedModel.isEmpty ? (preset.models.first ?? "") : storedModel
        return (endpoint, KeychainStore.read(AIProvider.keyAccount(providerId)) ?? "", model)
    }

    #if DEBUG
    /// README shots: a reply recorded from a real call (`cargo xtask shots`) is replayed from the environment, so no tokens are spent.
    static func replay(_ kind: String) -> String? { ProcessInfo.processInfo.environment["SWIMLIE_AI_\(kind)"].flatMap { $0.isEmpty ? nil : $0 } }
    /// Real replies are written to Documents so xtask can keep them.
    static func record(_ kind: String, _ text: String) {
        guard let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { return }
        try? text.write(to: docs.appendingPathComponent("ai-\(kind.lowercased()).txt"), atomically: true, encoding: .utf8)
    }
    #endif

    /// Plain-language review of one swim from its brief, by the on-device model or the chosen API.
    /// `onPartial` gets the text so far on the main actor as it streams in.
    static func review(brief: String, onPartial: @escaping @MainActor (String) -> Void) async throws -> String {
        #if DEBUG
        if let text = replay("REVIEW") { await onPartial(text); return text }
        #endif
        let d = UserDefaults.standard
        // Empty means the built-in prompt (Rust owns it).
        var customPrompt = d.string(forKey: "aiReviewPrompt") ?? ""
        // Reply in the app's language; the section labels stay English (the card matches and translates them).
        if let code = Bundle.main.preferredLocalizations.first, code != "en",
           let language = Locale(identifier: "en").localizedString(forLanguageCode: code) {
            let base = customPrompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? defaultReviewPrompt() : customPrompt
            customPrompt = base + " Write the review in \(language), but keep the five section labels exactly as written in English."
        }
        if d.string(forKey: "aiEngine") == "api" {
            let remote = remoteSettings()
            let sink = PartialSink(onPartial)
            let out = try await Task.detached(priority: .userInitiated) {
                try aiReviewStream(endpoint: remote.endpoint, apiKey: remote.key, model: remote.model, brief: brief, prompt: customPrompt, sink: sink)
            }.value
            #if DEBUG
            record("REVIEW", out)
            #endif
            return out
        }
        guard #available(iOS 26, *) else { throw AIAssistantError.unavailable(String(localized: "Needs iOS 26 or later")) }
        if let reason = unavailableReason { throw AIAssistantError.unavailable(reason) }
        let session = LanguageModelSession(instructions: customPrompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? defaultReviewPrompt() : customPrompt)
        var text = ""
        for try await snapshot in session.streamResponse(to: brief) {
            text = snapshot.content
            await onPartial(text)
        }
        return text
    }

    /// The JSON object in a reply: from its first "{" to its last "}", so code fences and prose around it are dropped.
    static func jsonObject(in reply: String) -> String {
        guard let start = reply.firstIndex(of: "{"), let end = reply.lastIndex(of: "}"), start <= end else { return reply }
        return String(reply[start...end])
    }
}

/// One request for a workout or plan. `system` holds the instructions and schema, `user` the form and the swimmer's
/// request. A rejected reply goes back with `fix`: the API gets the whole turn again, the on-device model keeps its session.
final class AIGeneration {
    private let system: String
    private let user: String
    private var session: Any? // LanguageModelSession (iOS 26), kept for the corrections

    init(system: String, user: String) { self.system = system; self.user = user }

    /// The raw reply; `AIAssistant.jsonObject(in:)` cleans it up.
    func ask() async throws -> String {
        #if DEBUG
        if let text = AIAssistant.replay("GENERATE") { return text }
        #endif
        let (system, user) = (system, user)
        if UserDefaults.standard.string(forKey: "aiEngine") == "api" {
            let remote = AIAssistant.remoteSettings()
            let out = try await Task.detached(priority: .userInitiated) {
                try aiReview(endpoint: remote.endpoint, apiKey: remote.key, model: remote.model, brief: user, prompt: system)
            }.value
            #if DEBUG
            AIAssistant.record("GENERATE", out)
            #endif
            return out
        }
        guard #available(iOS 26, *) else { throw AIAssistantError.unavailable(String(localized: "Needs iOS 26 or later")) }
        if let reason = AIAssistant.unavailableReason { throw AIAssistantError.unavailable(reason) }
        let s = LanguageModelSession(instructions: system)
        session = s
        return try await s.respond(to: user).content
    }

    /// A corrected reply for `reply`, which was rejected with `error`.
    func fix(reply: String, error: String) async throws -> String {
        #if DEBUG
        if let text = AIAssistant.replay("GENERATE") { return text }
        #endif
        let (system, user) = (system, user)
        if UserDefaults.standard.string(forKey: "aiEngine") == "api" {
            let remote = AIAssistant.remoteSettings()
            return try await Task.detached(priority: .userInitiated) {
                try aiFix(endpoint: remote.endpoint, apiKey: remote.key, model: remote.model, fix: AiFix(system: system, user: user, reply: reply, error: error))
            }.value
        }
        guard #available(iOS 26, *), let s = session as? LanguageModelSession else { throw AIAssistantError.unavailable(String(localized: "Needs iOS 26 or later")) }
        return try await s.respond(to: aiFixPrompt(error: error)).content
    }
}
