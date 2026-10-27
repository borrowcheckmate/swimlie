import CryptoKit
import Foundation
import SwimlieCore
import WatchConnectivity

// shortcut: sends the whole store on every change; add a watermark if payloads get large.
enum Sync {
    /// Set at app launch on each device, before WCSession can deliver a file.
    static var store: Store?
    static let changed = Notification.Name("SyncChanged") // posted on the main queue after a received payload is applied

    static func send() {
        guard WCSession.default.activationState == .activated, let store else { return }
        // Each export holds the whole store, so an unsent older export is superseded.
        for transfer in WCSession.default.outstandingFileTransfers where transfer.file.metadata?["kind"] as? String == "sync" {
            transfer.cancel()
        }
        guard let json = try? store.exportChanges() else { return }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("sync-\(UUID().uuidString).json")
        do { try json.write(to: url, atomically: true, encoding: .utf8) } catch { print("sync: \(error)"); return }
        WCSession.default.transferFile(url, metadata: ["kind": "sync"])
    }

    /// Called from `session(_:didFinish:error:)`: deletes our own sent payload, leaving other files alone.
    static func finished(_ transfer: WCSessionFileTransfer) {
        let url = transfer.file.fileURL
        guard url.lastPathComponent.hasPrefix("sync-"), url.deletingLastPathComponent().standardizedFileURL == FileManager.default.temporaryDirectory.standardizedFileURL else { return }
        try? FileManager.default.removeItem(at: url)
    }

    /// Returns false if the file is not a sync payload. The file is deleted when the delegate method returns, so read it synchronously.
    static func receive(_ file: WCSessionFile) -> Bool {
        guard file.metadata?["kind"] as? String == "sync" else { return false }
        do {
            let json = try String(contentsOf: file.fileURL, encoding: .utf8)
            _ = try store?.applyChanges(json: json)
            #if os(iOS)
            if (try? store?.importCssTests()) ?? 0 > 0 { send() } // a synced CSS test swim sets the CSS, which goes back to the watch
            #endif
            DispatchQueue.main.async { NotificationCenter.default.post(name: changed, object: nil) }
        } catch { print("sync: \(error)") }
        return true
    }

    /// The id a bundled workout gets, derived from its file name so both devices agree.
    static func bundledId(_ fileName: String) -> String {
        Insecure.MD5.hash(data: Data(fileName.utf8)).withUnsafeBytes { UUID(uuid: $0.load(as: uuid_t.self)) }.uuidString
    }

    /// The CSS test sample is part of the app: the list does not offer to delete it.
    static let cssTestId = bundledId("css-test.json")

    /// Every launch: the bundled workouts under ids derived from their file names, so both devices share the id. A deleted sample keeps its row (tombstone), so it stays deleted; invalid files are skipped.
    static func seed(_ store: Store, urls: [URL]) {
        for url in urls {
            do {
                let workout = try parseWorkout(json: String(contentsOf: url, encoding: .utf8))
                _ = try store.seedWorkout(id: bundledId(url.lastPathComponent), workout: workout)
            } catch {
                print("seed: skipped \(url.lastPathComponent): \(error)")
            }
        }
    }

    /// Sends one workout. WatchConnectivity keeps only the latest context, so a new send replaces the old one.
    static func sendWorkout(_ w: StoredWorkout) throws {
        try WCSession.default.updateApplicationContext(["kind": "workout", "id": w.id, "json": workoutToJson(workout: w.workout)])
    }

    /// Stored in UserDefaults so the watch can still start the workout after a crash or an aborted swim.
    static func receiveWorkout(_ ctx: [String: Any]) {
        guard ctx["kind"] as? String == "workout", let id = ctx["id"] as? String, let json = ctx["json"] as? String else { return }
        DispatchQueue.main.async { UserDefaults.standard.set(id, forKey: "sentWorkoutId"); UserDefaults.standard.set(json, forKey: "sentWorkoutJSON") }
    }

    /// The id of the workout last sent to the watch (phone side).
    static var sentWorkoutId: String? { WCSession.default.applicationContext["id"] as? String }
}
