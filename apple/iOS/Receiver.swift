import Foundation
import WatchConnectivity

/// Spike 0b: receives swim NDJSON files from the watch and lists them from Documents.
final class Receiver: NSObject, ObservableObject, WCSessionDelegate {
    @Published var files: [URL] = []
    private let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]

    override init() {
        super.init()
        refresh()
        if WCSession.isSupported() {
            WCSession.default.delegate = self
            WCSession.default.activate()
        }
    }

    func refresh() {
        let all = (try? FileManager.default.contentsOfDirectory(at: docs, includingPropertiesForKeys: nil)) ?? []
        files = all.filter { $0.pathExtension == "ndjson" }.sorted { $0.lastPathComponent > $1.lastPathComponent }
    }

    func session(_ s: WCSession, didReceive file: WCSessionFile) {
        if Sync.receive(file) { return }
        // The incoming file is deleted when this method returns, so move it synchronously.
        let dest = docs.appendingPathComponent(file.fileURL.lastPathComponent)
        try? FileManager.default.removeItem(at: dest)
        try? FileManager.default.moveItem(at: file.fileURL, to: dest)
        DispatchQueue.main.async { self.refresh() }
    }

    func session(_ s: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) {
        if state == .activated { Sync.send() }
    }
    func session(_ s: WCSession, didFinish transfer: WCSessionFileTransfer, error: Error?) { Sync.finished(transfer) }
    func sessionDidBecomeInactive(_ s: WCSession) {}
    func sessionDidDeactivate(_ s: WCSession) { s.activate() }
}
