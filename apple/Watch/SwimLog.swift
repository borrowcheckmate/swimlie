import Foundation
import HealthKit
import WatchConnectivity

/// Debug log of a real swim: the planned workout, the clock, HealthKit's laps and events, pauses and manual marks, as NDJSON
/// (same line format as Recorder, so `cargo xtask laps` reads both). Sent to the phone when the swim ends.
final class SwimLog: @unchecked Sendable {
    private let io = DispatchQueue(label: "swimlog.io")
    private var handle: FileHandle?
    private var url: URL?
    private var seen = Set<String>()

    func open(workoutJSON: String, id: String) {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let u = docs.appendingPathComponent("swim-\(ISO8601DateFormatter().string(from: Date())).ndjson")
        FileManager.default.createFile(atPath: u.path, contents: nil)
        io.sync { handle = try? FileHandle(forWritingTo: u); url = u; seen = [] }
        write("clock", ["uptime": ProcessInfo.processInfo.systemUptime, "wall": Date().timeIntervalSince1970])
        write("workout", ["id": id, "workout": (try? JSONSerialization.jsonObject(with: Data(workoutJSON.utf8))) ?? workoutJSON])
    }

    func write(_ kind: String, _ fields: [String: Any]) {
        var d = fields
        d["kind"] = kind
        d["received_at"] = Date().timeIntervalSince1970
        guard var line = try? JSONSerialization.data(withJSONObject: d) else { return }
        line.append(0x0A)
        io.async { try? self.handle?.write(contentsOf: line) }
    }

    /// Logs HealthKit events not seen before (workoutEvents is cumulative).
    func events(_ events: [HKWorkoutEvent]) {
        for e in events {
            let key = PoolWorkout.key(e)
            guard io.sync(execute: { seen.insert(key).inserted }) else { continue }
            var d: [String: Any] = ["type_name": Self.name(e.type), "start": e.dateInterval.start.timeIntervalSince1970, "end": e.dateInterval.end.timeIntervalSince1970]
            if let s = PoolWorkout.strokeStyle(e) { d["stroke_style_name"] = "\(PoolWorkout.stroke(s))" }
            var meta: [String: String] = [:]
            for (k, v) in e.metadata ?? [:] { meta[k] = "\(v)" }
            d["metadata"] = meta
            write("event", d)
        }
    }

    /// Closes the file and sends it to the phone.
    func close() {
        let u: URL? = io.sync { try? handle?.close(); handle = nil; return url }
        if let u, WCSession.default.activationState == .activated { WCSession.default.transferFile(u, metadata: nil) }
    }

    private static func name(_ t: HKWorkoutEventType) -> String {
        switch t {
        case .lap: "lap"
        case .segment: "segment"
        case .marker: "marker"
        case .pause: "pause"
        case .resume: "resume"
        case .motionPaused: "motionPaused"
        case .motionResumed: "motionResumed"
        case .pauseOrResumeRequest: "pauseOrResumeRequest"
        @unknown default: "unknown"
        }
    }
}
