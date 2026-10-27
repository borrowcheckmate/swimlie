import Foundation

/// Presets for the API engine. Every provider speaks the OpenAI-compatible chat and models API.
struct AIProvider: Identifiable {
    let id: String
    let name: String
    let endpoint: String
    let models: [String]

    static let all: [AIProvider] = [
        AIProvider(id: "mistral", name: "Mistral", endpoint: "https://api.mistral.ai/v1",
                   models: ["mistral-large-latest", "mistral-small-latest"]),
        AIProvider(id: "openai", name: "OpenAI", endpoint: "https://api.openai.com/v1",
                   models: ["gpt-5", "gpt-5-mini"]),
        AIProvider(id: "anthropic", name: "Anthropic", endpoint: "https://api.anthropic.com/v1",
                   models: ["claude-sonnet-5-5", "claude-haiku-5-5", "claude-opus-5-5"]),
        AIProvider(id: "gemini", name: "Google Gemini", endpoint: "https://generativelanguage.googleapis.com/v1beta/openai",
                   models: ["gemini-2.5-flash", "gemini-2.5-pro"]),
        AIProvider(id: "openrouter", name: "OpenRouter", endpoint: "https://openrouter.ai/api/v1",
                   models: ["openai/gpt-5", "anthropic/claude-sonnet-5-5"]),
        AIProvider(id: "custom", name: String(localized: "Custom (OpenAI-compatible)"), endpoint: "", models: []),
    ]

    static func find(_ id: String) -> AIProvider { all.first { $0.id == id } ?? all[0] }

    /// One Keychain account per provider, so switching providers keeps each key.
    static func keyAccount(_ id: String) -> String { "aiApiKey.\(id)" }
}
