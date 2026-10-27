import SwimlieCore
import SwiftUI

/// The swim just saved, as the Workout app shows it: totals, heart rate zones, then each set.
struct SummaryView: View {
    let saved: Session?
    var workout: Workout?
    var error: String?
    let onDone: () -> Void

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    if let name = workout?.name { Text(name).font(.headline).foregroundStyle(Color.accent) }
                    if let saved {
                        let s = sessionSummary(session: saved, cssSPer100m: nil)
                        totals(s)
                        if !s.zoneS.isEmpty { zones(s.zoneS) }
                        setRows(saved)
                    }
                    if let error { Text(error).font(.footnote).foregroundStyle(.red) }
                    Button("Done", action: onDone).frame(maxWidth: .infinity).id("done")
                }
            }
            #if DEBUG
            .onAppear { if UserDefaults.standard.string(forKey: "demo") == "saved-bottom" { proxy.scrollTo("done") } } // screenshot of the scrolled view
            #endif
        }
        .themed()
    }

    private func totals(_ s: SessionSummary) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            stat(String(localized: "Distance"), metres(s.distanceM), .distance)
            Divider()
            stat(String(localized: "Total time"), mmss(s.elapsedS), .time)
            Divider()
            stat(String(localized: "Swim time"), mmss(s.swimS), .time)
            if let p = s.paceSPer100m { Divider(); stat(String(localized: "Avg pace"), pace(p), .pace) }
            if let bpm = s.avgBpm { Divider(); stat(String(localized: "Avg heart rate"), "\(Int(bpm.rounded())) bpm", .heart) }
            if let bpm = s.maxBpm { Divider(); stat(String(localized: "Max heart rate"), "\(Int(bpm.rounded())) bpm", .heart) }
            if let swolf = s.swolfAvg { Divider(); stat(String(localized: "SWOLF"), "\(Int(swolf.rounded()))", .efficiency) }
        }
    }

    private func stat(_ caption: String, _ value: String, _ metric: Metric) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(caption).font(.footnote).foregroundStyle(.secondary)
            Text(value).font(.system(.title2, design: .rounded, weight: .semibold).monospacedDigit())
                .foregroundStyle(metric.color).minimumScaleFactor(0.6).lineLimit(1)
                .accessibilityIdentifier("summary." + caption.lowercased().replacingOccurrences(of: " ", with: "_"))
        }
    }

    /// Time per zone, bars scaled to the longest one.
    private func zones(_ zoneS: [Double]) -> some View {
        let most = max(zoneS.max() ?? 0, 1)
        return VStack(alignment: .leading, spacing: 6) {
            Text("Heart rate zones").font(.footnote.bold()).foregroundStyle(Color.accent).padding(.top, 8)
            ForEach(zoneS.indices, id: \.self) { i in
                HStack(spacing: 6) {
                    Text("\(i + 1)").font(.system(.footnote, design: .rounded, weight: .heavy)).foregroundStyle(zoneColors[i]).frame(width: 12)
                    GeometryReader { g in Capsule().fill(zoneColors[i]).frame(width: g.size.width * zoneS[i] / most) }.frame(height: 8)
                    Text(mmss(zoneS[i])).font(.system(.footnote, design: .rounded).monospacedDigit()).frame(width: 40, alignment: .trailing)
                }
            }
        }
    }

    private func setRows(_ saved: Session) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Sets").font(.footnote.bold()).foregroundStyle(Color.accent).padding(.top, 8)
            ForEach(Array(sets(session: saved).enumerated()), id: \.offset) { _, m in
                VStack(alignment: .leading, spacing: 1) {
                    Text(workout?.set(at: Position(block: m.block, round: m.round, set: m.set, rep: 0))?.title ?? String(localized: "Set"))
                        .font(.system(.footnote, design: .rounded, weight: .bold))
                    Text(pace(m.paceSPer100m) + (m.goalReps > 0 ? String(localized: "  \(m.goalHits)/\(m.goalReps) on goal") : ""))
                        .font(.footnote).foregroundStyle(.secondary)
                }
            }
        }
    }
}
