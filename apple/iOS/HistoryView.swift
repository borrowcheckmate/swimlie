import SwimlieCore
import SwiftUI

struct HistoryView: View {
    @SwiftUI.State private var sessions: [StoredSession] = []
    @SwiftUI.State private var css: Double?
    @SwiftUI.State private var path: [String] = []
    @SwiftUI.State private var showTrends = false
    @SwiftUI.State private var error: String?

    var body: some View {
        NavigationStack(path: $path) {
            List {
                if !sessions.isEmpty {
                    Section {
                        Button { showTrends = true } label: {
                            HStack(spacing: 12) {
                                IconBadge(systemName: "chart.xyaxis.line", metric: .pace)
                                Text("Trends").font(.headline)
                            }
                        }
                    }
                }
                ForEach(months, id: \.start) { month in
                    Section(month.start.formatted(.dateTime.month(.wide).year())) {
                        ForEach(month.sessions, id: \.id) { s in
                            NavigationLink(value: s.id) { SessionRow(stored: s, css: css) }
                                .swipeActions {
                                    Button(role: .destructive) { delete(s.id) } label: { Label("Delete", systemImage: "trash") }
                                }
                        }
                    }
                }
            }
            .overlay {
                if sessions.isEmpty {
                    ContentUnavailableView("No swims yet", systemImage: "figure.pool.swim",
                                           description: Text("Swim with Swimlie on your Apple Watch; your swims sync here."))
                }
            }
            .navigationTitle("History")
            .navigationDestination(for: String.self) { SessionDetailView(id: $0) }
            .navigationDestination(isPresented: $showTrends) { TrendsView() }
            .refreshable { reload() }
            .errorAlert($error)
            .onAppear {
                reload()
                #if DEBUG
                if UserDefaults.standard.string(forKey: "demo")?.hasPrefix("detail") == true, path.isEmpty, let id = sessions.first?.id { path = [id] }
                if UserDefaults.standard.string(forKey: "demo")?.hasPrefix("trends") == true { showTrends = true }
                #endif
            }
            .onReceive(NotificationCenter.default.publisher(for: Sync.changed)) { _ in reload() }
        }
    }

    /// Sessions grouped by calendar month, newest first.
    private var months: [(start: Date, sessions: [StoredSession])] {
        var out: [(start: Date, sessions: [StoredSession])] = []
        for s in sessions.sorted(by: { startedAt($0) > startedAt($1) }) {
            let day = startedAt(s)
            let start = Calendar.current.dateInterval(of: .month, for: day)?.start ?? day
            if out.last?.start == start { out[out.count - 1].sessions.append(s) } else { out.append((start, [s])) }
        }
        return out
    }

    private func reload() {
        do {
            // 0 m swims (started and ended at once) are not worth listing.
            sessions = (try Sync.store?.listSessions() ?? []).filter { sessionSummary(session: $0.session, cssSPer100m: nil).distanceM > 0 }
            css = try Sync.store?.latestCssSPer100m()
        } catch { self.error = error.localizedDescription }
    }

    private func delete(_ id: String) {
        do { _ = try Sync.store?.deleteSession(id: id) } catch { self.error = error.localizedDescription; return }
        reload()
        Sync.send()
    }
}

private func startedAt(_ s: StoredSession) -> Date {
    Date(timeIntervalSince1970: Double(s.session.startedAtMs) / 1000)
}

private struct SessionRow: View {
    let stored: StoredSession
    let css: Double?

    var body: some View {
        let s = sessionSummary(session: stored.session, cssSPer100m: css)
        HStack(spacing: 12) {
            IconBadge(systemName: "figure.pool.swim", metric: .distance)
            VStack(alignment: .leading, spacing: 2) {
                Text(stored.session.workout.name).font(.headline)
                HStack(alignment: .firstTextBaseline, spacing: 3) {
                    Text("\(s.distanceM)").font(.system(.title2, design: .rounded).weight(.semibold))
                    Text("M").font(.system(.title3, design: .rounded).weight(.semibold))
                }
                .foregroundStyle(Metric.distance.color)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 2) {
                Text(startedAt(stored).formatted(.dateTime.weekday(.abbreviated).day().month(.twoDigits)))
                    .font(.footnote).foregroundStyle(.secondary)
                if let p = s.paceSPer100m {
                    Text("\(clock(p)) /100m").font(.footnote).foregroundStyle(Metric.pace.color)
                }
            }
        }
    }
}

#if DEBUG
/// Synthetic sessions for the simulator, since simctl cannot tap through the app.
enum Demo {
    static let json = """
    {"version":1,"name":"Demo main set","pool_length_m":25,"blocks":[{"name":"Main","repeat":1,"sets":[
      {"reps":4,"distance_m":50,"goal_s":50},{"reps":2,"distance_m":100,"goal_s":110}]}]}
    """

    /// `-demo swim` replaces the synthetic sessions with the labelled laps of core/tests/data/recorded-swim-b.
    /// Copy laps.json and workout.json into the app's Documents folder first (simctl get_app_container).
    static func seedFromTestData(_ store: Store) {
        guard let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first,
              let lapsData = try? Data(contentsOf: docs.appendingPathComponent("laps.json")),
              let workoutText = try? String(contentsOf: docs.appendingPathComponent("workout.json"), encoding: .utf8),
              let root = try? JSONSerialization.jsonObject(with: lapsData) as? [String: Any],
              let rows = root["laps"] as? [[String: Any]],
              let started = root["started_at"] as? Double
        else { return }
        for s in (try? store.listSessions()) ?? [] { _ = try? store.deleteSession(id: s.id) }
        let workout = try! parseWorkout(json: workoutText)
        var session = Session(workout: workout, startedAtMs: Int64(started * 1000), workoutId: nil,
                              laps: [], lapPositions: [], lapLengths: [], hr: [], ageYears: 40, location: nil)
        for row in rows {
            // Only the labelled laps: unlabelled ones have no place in the workout.
            guard let blockName = row["block"] as? String, let block = workout.blocks.firstIndex(where: { $0.name == blockName }),
                  let set = row["set"] as? Int, let rep = row["rep"] as? Int,
                  let startS = row["start_s"] as? Double, let endS = row["end_s"] as? Double
            else { continue }
            let swolf = row["swolf"] as? Double // nil for kickboard laps, which have no stroke count
            let stroke: Stroke = switch row["stroke"] as? String {
            case "backstroke": .backstroke
            case "breaststroke": .breaststroke
            case "butterfly": .butterfly
            case "kickboard": .kickboard
            default: .freestyle
            }
            // SWOLF is time plus strokes for one length, so the strokes are what is left after the time.
            let strokes = swolf.map { UInt32(max(1, ($0 - (endS - startS)).rounded())) }
            session.laps.append(Lap(startS: startS, endS: endS, stroke: stroke, strokes: strokes))
            session.lapPositions.append(Position(block: UInt32(block), round: 0, set: UInt32(set - 1), rep: UInt32(rep - 1)))
            session.lapLengths.append(1)
        }
        // Real heart rate from the raw recording (hr.json, extracted from its HeartRate stats). Without it, a made-up wave for the UI.
        if let hrData = try? Data(contentsOf: docs.appendingPathComponent("hr.json")),
           let samples = try? JSONSerialization.jsonObject(with: hrData) as? [[String: Any]] {
            for s in samples {
                guard let t = s["t_s"] as? Double, t >= 0, let bpm = s["bpm"] as? Double ?? (s["bpm"] as? Int).map(Double.init) else { continue }
                session.hr.append(HrSample(tS: t, bpm: bpm))
            }
        } else if let end = session.laps.last?.endS {
            for at in stride(from: 0.0, through: end, by: 5) {
                session.hr.append(HrSample(tS: at, bpm: 135 + 30 * sin(at / 90)))
            }
        }
        _ = try? store.saveSession(id: nil, session: session)
    }

    static func seed(_ store: Store) {
        // Reseed when no session has heart rate yet (older demo data has none).
        guard let old = try? store.listSessions(), !old.contains(where: { !$0.session.hr.isEmpty }) else { return }
        for s in old { _ = try? store.deleteSession(id: s.id) }
        let workouts = ["Endurance 1500 m": "[{\"reps\":5,\"distance_m\":200,\"goal_s\":220},{\"reps\":2,\"distance_m\":250,\"goal_s\":275}]",
                        "Technique 1200 m": "[{\"reps\":8,\"distance_m\":50,\"goal_s\":58},{\"reps\":4,\"distance_m\":200,\"goal_s\":230}]",
                        "Threshold 100s": "[{\"reps\":10,\"distance_m\":100,\"goal_s\":108},{\"reps\":4,\"distance_m\":50,\"goal_s\":52}]",
                        "Steady 200s": "[{\"reps\":6,\"distance_m\":200,\"goal_s\":225}]"]
        let names = ["Steady 200s", "Threshold 100s", "Technique 1200 m", "Endurance 1500 m"]
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        for k in 0..<4 {
            let workout = try! parseWorkout(json: "{\"version\":1,\"name\":\"\(names[k])\",\"pool_length_m\":25,\"blocks\":[{\"name\":\"Main\",\"repeat\":1,\"sets\":\(workouts[names[k]]!)}]}")
            var session = Session(workout: workout, startedAtMs: now - Int64(3 - k) * 3 * 86_400_000, workoutId: nil,
                                  laps: [], lapPositions: [], lapLengths: [], hr: [], ageYears: 40,
                                  location: k == 3 ? Location(lat: 52.3731, lon: 4.8926, accuracyM: 25) : nil) // newest: shows the pool header
            var t = 0.0
            var reps: [(start: Double, end: Double)] = []
            for (si, set) in workout.blocks[0].sets.enumerated() {
                for rep in 0..<Int(set.reps) {
                    if !session.laps.isEmpty { t += 20 } // rest between reps
                    let start = t
                    for lap in 0..<Int(set.distanceM / workout.poolLengthM) {
                        let end = t + 28 - Double(k) + Double(lap % 3)
                        session.laps.append(Lap(startS: t, endS: end, stroke: .freestyle, strokes: 18))
                        session.lapPositions.append(Position(block: 0, round: 0, set: UInt32(si), rep: UInt32(rep)))
                        session.lapLengths.append(1)
                        t = end
                    }
                    reps.append((start, t))
                }
            }
            // Synthetic HR every 5 s: ramps up through each rep, decays towards 115 during rests.
            for at in stride(from: 0.0, through: t, by: 5) {
                let wobble = Double(k) * 2 + 2 * sin(at / 7)
                let bpm: Double
                if let r = reps.first(where: { at >= $0.start && at < $0.end }) {
                    bpm = 125 + 30 * (at - r.start) / (r.end - r.start) + wobble
                } else {
                    let rest = reps.last(where: { $0.end <= at }).map { 115 + 40 * exp(-(at - $0.end) / 15) } ?? 115
                    bpm = rest + wobble
                }
                session.hr.append(HrSample(tS: at, bpm: bpm))
            }
            _ = try! store.saveSession(id: nil, session: session)
        }
    }
}
#endif
