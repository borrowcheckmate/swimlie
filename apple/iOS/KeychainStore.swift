import Foundation
import Security

/// Keeps small secrets (the AI API key) in the iOS Keychain. Never UserDefaults.
enum KeychainStore {
    private static func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "swimlie",
         kSecAttrAccount as String: account]
    }

    static func read(_ account: String) -> String? {
        #if DEBUG
        // Debug only: a key passed in the launch environment (from .env, see `cargo xtask`) stands in for any provider's key.
        if account.hasPrefix("aiApiKey."), let key = ProcessInfo.processInfo.environment["SWIMLIE_API_KEY"], !key.isEmpty { return key }
        #endif
        var q = query(account)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: AnyObject?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    @discardableResult
    static func write(_ account: String, _ value: String) -> Bool {
        let base = query(account)
        guard !value.isEmpty else { return delete(account) }
        let attrs: [String: Any] = [kSecValueData as String: Data(value.utf8),
                                    kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        if SecItemUpdate(base as CFDictionary, attrs as CFDictionary) == errSecSuccess { return true }
        var add = base
        add.merge(attrs) { _, new in new }
        return SecItemAdd(add as CFDictionary, nil) == errSecSuccess
    }

    @discardableResult
    static func delete(_ account: String) -> Bool {
        let status = SecItemDelete(query(account) as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }
}
