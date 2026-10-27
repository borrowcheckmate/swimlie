import Charts
import CoreLocation
import MapKit
import SwimlieCore
import SwiftUI

struct SessionDetailView: View {
    let id: String
    @State private var stored: StoredSession?
    @State private var css: Double?
    @State private var error: String?
    @AppStorage("aiEngine") private var aiEngine = "on-device"
    @AppStorage("aiProvider") private var aiProvider = "mistral"
    @AppStorage("aiEndpoint") private var aiEndpoint = "https://api.mistral.ai/v1"
    @AppStorage("aiModel") private var aiModel = ""
    @State private var aiText: String?
    @State private var aiBusy = false
    @State private var aiError: String?

    var body: some View {
        if let stored {
            let session = stored.session
            let s = sessionSummary(session: session, cssSPer100m: css)
            let allReps = reps(session: session)
            let allLaps = laps(session: session)
            let allSets = sets(session: session)
            let showStrokes = s.byStroke.count > 1 || s.byStroke.contains { $0.stroke != .any }
            let start = Date(timeIntervalSince1970: Double(session.startedAtMs) / 1000)
            let end = start.addingTimeInterval(s.elapsedS)
            let stats: [Stat] = [
                Stat(title: String(localized: "Swim time"), value: clock(s.swimS), metric: .time),
                Stat(title: String(localized: "Elapsed"), value: clock(s.elapsedS), metric: .time),
                Stat(title: String(localized: "Distance"), value: "\(s.distanceM)", unit: "M", metric: .distance),
                Stat(title: String(localized: "Pace"), value: s.paceSPer100m.map(clock), unit: "/100M", metric: .pace),
                Stat(title: String(localized: "Avg Heart Rate"), value: s.avgBpm.map(whole), unit: "BPM", metric: .heart),
                Stat(title: String(localized: "Max Heart Rate"), value: s.maxBpm.map(whole), unit: "BPM", metric: .heart),
                Stat(title: String(localized: "Rest"), value: clock(s.restS), metric: .time),
                Stat(title: String(localized: "Lengths"), value: "\(s.laps)", metric: .distance),
                Stat(title: String(localized: "SWOLF"), value: s.swolfAvg.map(whole), metric: .efficiency),
                Stat(title: String(localized: "Distance/Stroke"), value: s.distancePerStrokeM.map { String(format: "%.2f", $0) }, unit: "M", metric: .efficiency),
                Stat(title: String(localized: "Stroke Rate"), value: s.strokeRatePerMin.map(whole), unit: "SPM", metric: .strokes),
                Stat(title: String(localized: "Strokes"), value: s.strokes.map { "\($0)" }, metric: .strokes),
                Stat(title: String(localized: "Training Load"), value: s.tss.map(whole), metric: .load),
            ]
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        VStack(alignment: .leading, spacing: 8) {
                            IconBadge(systemName: "figure.pool.swim", metric: .distance)
                                .scaleEffect(1.5).frame(width: 66, height: 66)
                            Text(session.workout.name).font(.title2.bold())
                            Text("\(start.formatted(date: .abbreviated, time: .omitted)), \(start.formatted(date: .omitted, time: .shortened))–\(end.formatted(date: .omitted, time: .shortened)) · \(session.workout.poolLengthM) m pool")
                                .font(.subheadline).foregroundStyle(.secondary)
                            if let location = session.location { PoolPlace(location: location) }
                        }
                        if aiBusy || aiText != nil || aiError != nil {
                            AIReviewCard(text: aiText, busy: aiBusy, error: aiError, reason: reason,
                                         note: aiEngine == "api" && reason == nil ? String(localized: "Sends a summary of this swim to \(AIProvider.find(aiProvider).name).") : nil,
                                         redo: { runReview(stored) })
                                .id("ai")
                        }
                        titled(String(localized: "Workout Details")) {
                            StatGrid(stats: stats).card()
                        }
                        if !session.hr.isEmpty {
                            titled(String(localized: "Heart Rate")) {
                                VStack(alignment: .leading, spacing: 16) {
                                    if let avg = s.avgBpm {
                                        Text("\(whole(avg)) BPM AVG")
                                            .font(.system(.title2, design: .rounded).weight(.semibold))
                                            .foregroundStyle(Metric.heart.color)
                                    }
                                    HeartRateChart(hr: session.hr, laps: allLaps)
                                    if !s.zoneS.isEmpty { ZoneRows(zones: s.zoneS) }
                                }
                                .card()
                            }
                            .id("hr")
                        }
                        if !allLaps.isEmpty {
                            titled(String(localized: "Pace per Length")) {
                                PaceChart(laps: allLaps, byStroke: s.byStroke.count > 1).card()
                            }
                            .id("pace")
                        }
                        if showStrokes {
                            titled(String(localized: "Strokes")) {
                                VStack(alignment: .leading, spacing: 16) {
                                    ForEach(s.byStroke, id: \.stroke) { x in
                                        HStack(alignment: .firstTextBaseline) {
                                            VStack(alignment: .leading, spacing: 2) {
                                                Text(x.stroke.name).font(.headline)
                                                Text("\(x.lengths) lengths").font(.footnote).foregroundStyle(.secondary)
                                            }
                                            Spacer()
                                            HStack(spacing: 12) {
                                                Text("\(x.distanceM) m").foregroundStyle(Metric.distance.color)
                                                Text("\(clock(x.paceSPer100m))/100m").foregroundStyle(Metric.pace.color)
                                            }
                                            .font(.system(.body, design: .rounded).monospacedDigit())
                                        }
                                    }
                                }
                                .card()
                            }
                        }
                        titled(String(localized: "Sets")) {
                            ForEach(allSets.indices, id: \.self) { i in
                                let m = allSets[i]
                                let setReps = allReps.filter { $0.pos.block == m.block && $0.pos.round == m.round && $0.pos.set == m.set }
                                VStack(alignment: .leading, spacing: 12) {
                                    HStack {
                                        Text(setTitle(session.workout, m)).font(.headline)
                                        Spacer()
                                        Text("\(clock(m.paceSPer100m))/100m").font(.headline).monospacedDigit().foregroundStyle(Metric.pace.color)
                                    }
                                    SetStats(m: m)
                                    ForEach(setReps.indices, id: \.self) { k in
                                        if k > 0 { Divider() }
                                        RepRow(rep: setReps[k], laps: allLaps.filter { $0.pos == setReps[k].pos },
                                               open: demoOpenFirstRep && i == 0 && setReps[k].pos.rep == 0)
                                    }
                                }
                                .card()
                                .id("set\(i)")
                            }
                        }
                    }
                    .padding()
                }
                .background(Color(.systemBackground))
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button {
                            if aiBusy || aiText != nil || aiError != nil {
                                withAnimation { proxy.scrollTo("ai", anchor: .top) }
                            } else {
                                runReview(stored)
                            }
                        } label: {
                            Image(systemName: "sparkles")
                        }
                        .accessibilityLabel("Review with AI")
                        .disabled(aiBusy)
                    }
                }
                .onAppear {
                    #if DEBUG
                    // simctl cannot scroll, so -demo detail-hr / detail-pace / detail-sets jump to a section.
                    if let id = ["detail-hr": "hr", "detail-pace": "pace", "detail-sets": "set0"][UserDefaults.standard.string(forKey: "demo") ?? ""] {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { proxy.scrollTo(id, anchor: .top) }
                    }
                    #endif
                }
            }
            .errorAlert($error)
        } else {
            ContentUnavailableView("Session not found", systemImage: "exclamationmark.triangle")
                .onAppear {
                    reload()
                    #if DEBUG
                    if demoArg == "detail-review", aiText == nil, let stored { runReview(stored) } // README shot: a real AI review
                    #endif
                }
                .errorAlert($error)
        }
    }

    private var reason: String? {
        AIAssistant.reason(engine: aiEngine, endpoint: aiEndpoint, providerId: aiProvider, model: aiModel)
    }

    private func reload() {
        do {
            stored = try Sync.store?.getSession(id: id)
            css = try Sync.store?.latestCssSPer100m()
            aiText = UserDefaults.standard.string(forKey: "aiReview.\(id)")
        } catch { self.error = error.localizedDescription }
    }

    private func runReview(_ stored: StoredSession) {
        if let reason {
            aiError = reason
            return
        }
        aiBusy = true
        aiError = nil
        Task {
            do {
                let out = try await AIAssistant.review(brief: try brief(stored)) { partial in
                    aiText = partial
                }
                aiText = out
                UserDefaults.standard.set(out, forKey: "aiReview.\(stored.id)")
            } catch {
                aiError = "AI could not review this swim: \(error.localizedDescription)"
            }
            aiBusy = false
        }
    }

    /// The swim, the swimmer's profile and every earlier swim (Rust ignores 0 m swims and compares the newest 5), as the plain-text brief.
    private func brief(_ stored: StoredSession) throws -> String {
        let all = try Sync.store?.listSessions() ?? []
        let start = stored.session.startedAtMs
        let previous = all.filter { $0.id != stored.id && $0.session.startedAtMs < start }
            .sorted { $0.session.startedAtMs > $1.session.startedAtMs }
        let profile = swimmerProfile(sessions: all, cssSPer100m: try? Sync.store?.latestCssSPer100m(),
                                     nowMs: Int64(Date().timeIntervalSince1970 * 1000),
                                     level: swimmerLevel(stored: UserDefaults.standard.string(forKey: "swimLevel") ?? ""))
        return sessionReviewBrief(session: stored, profile: profile, previous: previous)
    }
}

/// The AI review section: spinner, text, error and unavailable reason, with Redo.
private struct AIReviewCard: View {
    let text: String?
    let busy: Bool
    let error: String?
    let reason: String?
    let note: String?
    let redo: () -> Void
    @State private var open = false

    /// Stray markdown (e.g. **bold**) renders as bold, and each of the five labels is bold. Line breaks are kept. Falls back to plain text.
    private func reviewText(_ text: String) -> AttributedString {
        let labels = ["Summary", "Achievements", "Progress", "What to improve", "Next time"]
        var out = AttributedString()
        for (i, line) in text.components(separatedBy: "\n").enumerated() {
            if i > 0 { out += AttributedString("\n") }
            var part = (try? AttributedString(markdown: line, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(line)
            // "Label:" at the start of a line, or the label alone on its line; shown in the app's language.
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if let label = labels.first(where: { trimmed == $0 || line.hasPrefix($0 + ":") }),
               let r = part.range(of: line.hasPrefix(label + ":") ? label + ":" : label) {
                part[r].font = Font.body.bold()
                part.replaceSubrange(r, with: AttributedString(String(localized: String.LocalizationValue(label)) + (line.hasPrefix(label + ":") ? ":" : ""), attributes: part[r].runs.first?.attributes ?? AttributeContainer()))
            }
            out += part
        }
        return out
    }

    @State private var fullHeight: CGFloat = 0
    private let peekHeight: CGFloat = 96

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button {
                withAnimation(.easeInOut(duration: 0.3)) { open.toggle() }
            } label: {
                HStack {
                    Text("AI Review").font(.title3.bold())
                    Spacer()
                    Image(systemName: "chevron.right").rotationEffect(.degrees(open ? 90 : 0)).foregroundStyle(.secondary)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(open ? "Collapse AI Review" : "Expand AI Review")
            VStack(alignment: .leading, spacing: 8) {
                VStack(alignment: .leading, spacing: 12) {
                    if let text {
                        // Collapsed: the first lines fade out; the full height is measured so the change animates.
                        Text(reviewText(text))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .topLeading)
                            .fixedSize(horizontal: false, vertical: true)
                            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { fullHeight = $0 }
                            .frame(height: open || fullHeight == 0 ? nil : min(peekHeight, fullHeight), alignment: .top)
                            .clipped()
                            .mask(
                                LinearGradient(stops: [.init(color: .black, location: 0),
                                                       .init(color: .black, location: open || fullHeight <= peekHeight ? 1 : 0.35),
                                                       .init(color: open || fullHeight <= peekHeight ? .black : .clear, location: 1)],
                                               startPoint: .top, endPoint: .bottom))
                            .contentShape(Rectangle())
                            .onTapGesture { if !open { withAnimation(.easeInOut(duration: 0.3)) { open = true } } }
                    }
                    if let error { Text(error).font(.footnote).foregroundStyle(.red) }
                    if let reason { Text(reason).font(.footnote).foregroundStyle(.secondary) }
                    if open || text == nil || busy {
                        HStack {
                            Button(action: redo) {
                                Label("Redo", systemImage: "sparkles")
                            }
                            .buttonStyle(.bordered)
                            .disabled(busy || reason != nil)
                            if busy { ProgressView().padding(.leading, 8) }
                        }
                        .transition(.opacity)
                    }
                }
                .card()
                if open, let note {
                    Text(note).font(.footnote).foregroundStyle(.secondary).transition(.opacity)
                }
            }
        }
        .animation(.easeInOut(duration: 0.3), value: open)
        // A review being written opens the card so it can be watched.
        .onChange(of: busy) { _, now in if now { open = true } }
    }
}

private struct Stat {
    let title: String
    let value: String?
    var unit: String? = nil
    let metric: Metric
}

/// Two columns of stat tiles, rows separated by dividers; missing values are omitted.
private struct StatGrid: View {
    let stats: [Stat]

    var body: some View {
        let shown = stats.filter { $0.value != nil }
        let rows = stride(from: 0, to: shown.count, by: 2).map { Array(shown[$0..<min($0 + 2, shown.count)]) }
        VStack(spacing: 0) {
            ForEach(rows.indices, id: \.self) { r in
                if r > 0 { Divider().padding(.vertical, 12) }
                HStack(alignment: .top, spacing: 16) {
                    ForEach(rows[r], id: \.title) { st in
                        if let value = st.value {
                            StatTile(title: st.title, value: value, unit: st.unit, metric: st.metric)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
            }
        }
    }
}

/// Pool name from the start position (shown once reverse geocoding answers), and a small map of the spot.
private struct PoolPlace: View {
    let location: Location
    @State private var name: String?

    var body: some View {
        let spot = CLLocationCoordinate2D(latitude: location.lat, longitude: location.lon)
        VStack(alignment: .leading, spacing: 12) {
            if let name { Text(name).font(.headline) }
            Map(initialPosition: .camera(MapCamera(centerCoordinate: spot, distance: 800))) {
                Marker("", coordinate: spot)
            }
            .frame(height: 140)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .allowsHitTesting(false)
        }
        .card()
        .task {
            let places = try? await CLGeocoder().reverseGeocodeLocation(CLLocation(latitude: location.lat, longitude: location.lon))
            if let place = places?.first { name = place.name ?? place.locality }
        }
    }
}

private struct HeartRateChart: View {
    let hr: [HrSample]
    let laps: [LapMetrics]

    var body: some View {
        let bpms = hr.map(\.bpm)
        let lo = (bpms.min() ?? 0) - 5, hi = (bpms.max() ?? 0) + 5
        let avg = bpms.reduce(0, +) / Double(bpms.count), peak = bpms.max() ?? 0
        Chart {
            // Swim bands go first so the line draws over them.
            ForEach(laps, id: \.index) { lap in
                RectangleMark(xStart: .value(String(localized: "Start"), lap.startS), xEnd: .value(String(localized: "End"), lap.startS + lap.swimS),
                              yStart: .value(String(localized: "Min"), lo), yEnd: .value(String(localized: "Max"), hi))
                    .foregroundStyle(.white.opacity(0.08))
            }
            ForEach(hr, id: \.tS) { h in
                LineMark(x: .value(String(localized: "Time"), h.tS), y: .value(String(localized: "Bpm"), h.bpm))
                    .foregroundStyle(Metric.heart.color)
            }
        }
        .chartYScale(domain: lo...hi)
        .chartXAxis {
            AxisMarks { value in
                AxisGridLine()
                AxisValueLabel { if let s = value.as(Double.self) { Text(clock(s)).foregroundStyle(.secondary) } }
            }
        }
        .frame(height: 180)
        .accessibilityLabel("Heart rate over time")
        .accessibilityValue("Average \(whole(avg)), max \(whole(peak)) beats per minute")
    }
}

/// Seconds in each heart-rate zone, as a bar proportional to the session's total zone time.
private struct ZoneRows: View {
    let zones: [Double]

    var body: some View {
        let total = zones.reduce(0, +)
        VStack(spacing: 10) {
            ForEach(zones.indices, id: \.self) { i in
                HStack(spacing: 12) {
                    Text("Zone \(i + 1)").font(.footnote).frame(width: 52, alignment: .leading)
                    GeometryReader { g in
                        ZStack(alignment: .leading) {
                            Capsule().fill(.white.opacity(0.1))
                            Capsule().fill(zoneColors[i]).frame(width: g.size.width * (total > 0 ? zones[i] / total : 0))
                        }
                    }
                    .frame(height: 8)
                    Text(clock(zones[i])).font(.footnote.monospacedDigit()).frame(width: 52, alignment: .trailing)
                }
                .opacity(zones[i] > 0 ? 1 : 0.35)
            }
        }
    }
}

private struct PaceChart: View {
    let laps: [LapMetrics]
    let byStroke: Bool

    var body: some View {
        let paces = laps.map(\.paceSPer100m)
        // Bars would otherwise start at zero, outside the range; they start at the floor instead.
        let lo = (paces.min() ?? 0) - 5, hi = (paces.max() ?? 0) + 5
        Chart {
            ForEach(laps, id: \.index) { lap in
                if byStroke {
                    BarMark(x: .value(String(localized: "Length"), Int(lap.index) + 1), yStart: .value(String(localized: "Base"), lo), yEnd: .value(String(localized: "Pace"), lap.paceSPer100m))
                        .foregroundStyle(by: .value(String(localized: "Stroke"), lap.stroke.name))
                        .accessibilityLabel("Length \(Int(lap.index) + 1)").accessibilityValue(spokenPace(lap.paceSPer100m))
                } else {
                    BarMark(x: .value(String(localized: "Length"), Int(lap.index) + 1), yStart: .value(String(localized: "Base"), lo), yEnd: .value(String(localized: "Pace"), lap.paceSPer100m))
                        .foregroundStyle(Metric.pace.color)
                        .accessibilityLabel("Length \(Int(lap.index) + 1)").accessibilityValue(spokenPace(lap.paceSPer100m))
                }
            }
        }
        .chartYScale(domain: lo...hi)
        .chartYAxis {
            AxisMarks(values: .stride(by: paceStride(paces))) { value in
                AxisGridLine()
                AxisValueLabel { if let s = value.as(Double.self) { Text(clock(s)).foregroundStyle(.secondary) } }
            }
        }
        .frame(height: 180)
    }
}

#if DEBUG
/// `-demo detail-sets` opens the first rep, so its laps show in the screenshot.
private let demoOpenFirstRep = UserDefaults.standard.string(forKey: "demo") == "detail-sets"
#else
private let demoOpenFirstRep = false
#endif

/// Fastest, slowest and drift, then rest, HR and goal; missing parts are omitted.
private struct SetStats: View {
    let m: SetMetrics

    var body: some View {
        let drift = m.paceDriftSPer100m
        let driftText = (drift > 0 ? "+" : drift < 0 ? "−" : "") + String(format: "%.1f", abs(drift))
        let driftColor = drift > 0 ? Color.orange : drift < 0 ? Color.green : Color.secondary
        let hr = [m.avgBpm.map { String(localized: "\(Int($0.rounded())) avg") }, m.maxBpm.map { "\(Int($0.rounded())) max" }].compactMap { $0 }
        let second = [
            m.restS > 0 ? String(localized: "Rest \(clock(m.restS))") : nil,
            hr.isEmpty ? nil : "HR " + hr.joined(separator: " / "),
            m.goalReps > 0 ? String(localized: "Goal \(m.goalHits)/\(m.goalReps)") : nil,
        ].compactMap { $0 }.joined(separator: " · ")
        VStack(alignment: .leading) {
            (Text("Fastest \(clock(m.fastestPaceSPer100m)) · Slowest \(clock(m.slowestPaceSPer100m)) · Drift ")
                + Text("\(driftText) s/100m").foregroundStyle(driftColor))
            if !second.isEmpty { Text(second) }
        }
        .font(.footnote.monospacedDigit()).foregroundStyle(.secondary)
    }
}

/// "4×50 m freestyle", plus " · round N" when the block repeats.
private func setTitle(_ workout: Workout, _ m: SetMetrics) -> String {
    guard workout.blocks.indices.contains(Int(m.block)),
          workout.blocks[Int(m.block)].sets.indices.contains(Int(m.set)) else { return String(localized: "Set") }
    let block = workout.blocks[Int(m.block)]
    let set = block.sets[Int(m.set)]
    let round = block.repeat > 1 ? String(localized: " · round \(m.round + 1)") : ""
    let stroke = set.stroke == .any ? "" : " \(set.stroke.name)"
    return "\(set.reps)×\(set.distanceM) m\(stroke)\(round)"
}

private struct RepRow: View {
    let rep: RepMetrics
    let laps: [LapMetrics]
    @State private var open: Bool

    init(rep: RepMetrics, laps: [LapMetrics], open: Bool) {
        self.rep = rep
        self.laps = laps
        _open = State(initialValue: open)
    }

    var body: some View {
        let extra = [
            rep.swolfAvg.map { "SWOLF \(Int($0.rounded()))" },
            rep.strokeRatePerMin.map { "\(Int($0.rounded())) spm" },
            rep.distancePerStrokeM.map { String(localized: "\(String(format: "%.2f", $0)) m/slag") },
            rep.avgBpm.map { String(localized: "HR \(Int($0.rounded()))/\(Int((rep.maxBpm ?? $0).rounded()))") },
            rep.restHrDropBpm.map { Int($0.rounded()) }.flatMap { $0 == 0 ? nil : String(localized: "\($0 > 0 ? "−" : "+")\(abs($0))\u{00A0}bpm rust") },
        ].compactMap { $0 }.joined(separator: " · ")
        DisclosureGroup(isExpanded: $open) {
            if !laps.isEmpty {
                Grid(alignment: .trailing, horizontalSpacing: 12, verticalSpacing: 6) {
                    GridRow {
                        Text("#").gridColumnAlignment(.leading)
                        Text("Time"); Text("Pace"); Text("Str"); Text("SWOLF"); Text("HR")
                    }
                    .foregroundStyle(.tertiary)
                    ForEach(laps, id: \.index) { lap in
                        GridRow {
                            Text("\(lap.index + 1)" + (lap.lengths > 1 ? " ×\(lap.lengths)" : "")).gridColumnAlignment(.leading)
                            Text(swimTime(lap.swimS))
                            Text(clock(lap.paceSPer100m))
                            Text(lap.strokes.map { "\($0)" } ?? "–")
                            Text(lap.swolf.map(whole) ?? "–")
                            Text(lap.avgBpm.map(whole) ?? "–")
                        }
                    }
                }
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
            }
        } label: {
            VStack(alignment: .leading) {
                HStack {
                    Text("Rep \(rep.pos.rep + 1)")
                    Spacer()
                    Text(swimTime(rep.swimS)).frame(width: 52, alignment: .trailing)
                    Text("\(clock(rep.paceSPer100m))/100m").fixedSize().frame(width: 96, alignment: .trailing)
                    if let d = rep.goalDeltaS {
                        Text(d > 0 ? String(format: "+%.1f s", d) : String(format: "−%.1f s", abs(d)))
                            .frame(width: 72, alignment: .trailing)
                            .foregroundStyle(d > 0 ? Color.orange : Color.green)
                    }
                }
                .font(.system(.body, design: .rounded).monospacedDigit())
                if !extra.isEmpty {
                    Text(extra).font(.footnote).foregroundStyle(.secondary)
                        .multilineTextAlignment(.leading).frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .tint(.primary) // the app-wide distance tint would otherwise colour the rep label
    }
}

/// m:ss.s, for a single rep's swim time.
func swimTime(_ seconds: Double) -> String {
    let t = (seconds * 10).rounded() / 10
    let m = Int(t) / 60
    return String(format: "%d:%04.1f", m, t - Double(m * 60))
}
