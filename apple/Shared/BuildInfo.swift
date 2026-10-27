import Foundation

/// The git commit the app was built from (GitCommit.txt in the app bundle, written by a post-build script).
enum BuildInfo {
    static let commit: String? = Bundle.main.url(forResource: "GitCommit", withExtension: "txt")
        .flatMap { try? String(contentsOf: $0, encoding: .utf8) }?.trimmingCharacters(in: .whitespacesAndNewlines)

    /// "1.0 (1)" from the bundle.
    static var version: String {
        let info = Bundle.main.infoDictionary
        let v = info?["CFBundleShortVersionString"] as? String ?? "?"
        let b = info?["CFBundleVersion"] as? String ?? "?"
        return "\(v) (\(b))"
    }

    /// "Build abc1234", or nil outside debug builds (the Library screen only shows it while developing).
    static var label: String? {
        #if DEBUG
        return commit.map { "Build \($0)" }
        #else
        return nil
        #endif
    }
}
