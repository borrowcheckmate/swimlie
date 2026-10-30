import Charts
import SwimlieCore
import SwiftUI

struct TrendsView: View {
    @SwiftUI.State private var trend: [Trend] = []
    @SwiftUI.State private var records: [PersonalRecord] = []
    @SwiftUI.State private var weeks: [WeekTotal] = []
    @SwiftUI.State private var error: String?

    var body: some View {
        // Each card's headline is the newest session that has that number.
        let pace = trend.last { $0.paceS != nil }?.paceS
        let week = weeks.last
        let hr = trend.last { $0.avgBpm != nil }?.avgBpm
        let swolf = trend.last { $0.swolfAvg != nil }?.swolfAvg
        let dps = trend.last { $0.distancePerStrokeM != nil }?.distancePerStrokeM
        let tss = trend.last { $0.tss != nil }?.tss
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    titled(String(localized: "Pace")) {
                        VStack(alignment: .leading, spacing: 16) {
                            StatTile(title: String(localized: "Latest pace"), value: pace.map(clock) ?? "–", unit: "/100M", metric: .pace)
                            Chart {
                                ForEach(trend) { t in
                                    if let p = t.paceS {
                                        LineMark(x: .value(String(localized: "Date"), t.date), y: .value(String(localized: "Pace"), p)).foregroundStyle(Metric.pace.color)
                                        PointMark(x: .value(String(localized: "Date"), t.date), y: .value(String(localized: "Pace"), p)).foregroundStyle(Metric.pace.color)
                                            .accessibilityLabel(t.label).accessibilityValue(spokenPace(p))
                                    }
                                }
                            }
                            .chartYScale(domain: .automatic(includesZero: false))
                            .chartYAxis {
                                AxisMarks(values: .stride(by: paceStride(trend.compactMap(\.paceS)))) { value in
                                    AxisGridLine()
                                    AxisValueLabel { if let s = value.as(Double.self) { Text(clock(s)) } }
                                }
                            }
                            .frame(height: 200)
                        }
                        .card()
                    }
                    titled(String(localized: "Distance per week")) {
                        VStack(alignment: .leading, spacing: 16) {
                            StatTile(title: week.map(weekLabel) ?? String(localized: "This week"), value: week.map { "\($0.distanceM)" } ?? "–", unit: "M", metric: .distance)
                            Chart(weeks, id: \.weekStartMs) { w in
                                BarMark(x: .value(String(localized: "Week"), weekDate(w), unit: .weekOfYear), y: .value(String(localized: "Distance"), Double(w.distanceM)))
                                    .foregroundStyle(Metric.distance.color)
                                    .accessibilityLabel(weekLabel(w)).accessibilityValue(String(localized: "\(w.distanceM.formatted()) metres"))
                            }
                            .frame(height: 200)
                        }
                        .card()
                    }
                    if trend.contains(where: { $0.avgBpm != nil }) {
                        titled(String(localized: "Heart rate")) {
                            VStack(alignment: .leading, spacing: 16) {
                                StatTile(title: String(localized: "Latest average"), value: hr.map(whole) ?? "–", unit: "BPM", metric: .heart)
                                Chart {
                                    ForEach(trend) { t in
                                        if let avg = t.avgBpm {
                                            LineMark(x: .value(String(localized: "Date"), t.date), y: .value(String(localized: "BPM"), avg))
                                                .foregroundStyle(by: .value(String(localized: "Series"), String(localized: "Avg")))
                                            PointMark(x: .value(String(localized: "Date"), t.date), y: .value(String(localized: "BPM"), avg))
                                                .foregroundStyle(by: .value(String(localized: "Series"), String(localized: "Avg")))
                                                .accessibilityLabel(t.label).accessibilityValue(String(localized: "Average \(whole(avg)) beats per minute"))
                                        }
                                        if let mx = t.maxBpm {
                                            LineMark(x: .value(String(localized: "Date"), t.date), y: .value(String(localized: "BPM"), mx))
                                                .foregroundStyle(by: .value(String(localized: "Series"), String(localized: "Max")))
                                            PointMark(x: .value(String(localized: "Date"), t.date), y: .value(String(localized: "BPM"), mx))
                                                .foregroundStyle(by: .value(String(localized: "Series"), String(localized: "Max")))
                                                .accessibilityLabel(t.label).accessibilityValue(String(localized: "Max \(whole(mx)) beats per minute"))
                                        }
                                    }
                                }
                                .chartForegroundStyleScale([String(localized: "Avg"): Metric.heart.color, String(localized: "Max"): Metric.heart.color.opacity(0.5)])
                                .chartYScale(domain: .automatic(includesZero: false))
                                .frame(height: 200)
                            }
                            .card()
                        }
                        .id("hr")
                    }
                    if trend.contains(where: { $0.swolfAvg != nil }) {
                        titled(String(localized: "SWOLF, lower is better")) {
                            VStack(alignment: .leading, spacing: 16) {
                                StatTile(title: String(localized: "Latest SWOLF"), value: swolf.map(whole) ?? "–", metric: .efficiency)
                                Chart {
                                    ForEach(trend) { t in
                                        if let v = t.swolfAvg {
                                            LineMark(x: .value(String(localized: "Date"), t.date), y: .value("SWOLF", v)).foregroundStyle(Metric.efficiency.color)
                                            PointMark(x: .value(String(localized: "Date"), t.date), y: .value("SWOLF", v)).foregroundStyle(Metric.efficiency.color)
                                                .accessibilityLabel(t.label).accessibilityValue("SWOLF \(whole(v))")
                                        }
                                    }
                                }
                                .chartYScale(domain: .automatic(includesZero: false))
                                .frame(height: 160)
                            }
                            .card()
                        }
                    }
                    if trend.contains(where: { $0.distancePerStrokeM != nil }) {
                        titled(String(localized: "Distance per stroke")) {
                            VStack(alignment: .leading, spacing: 16) {
                                StatTile(title: String(localized: "Latest"), value: dps.map { String(format: "%.2f", $0) } ?? "–", unit: "M", metric: .efficiency)
                                Chart {
                                    ForEach(trend) { t in
                                        if let d = t.distancePerStrokeM {
                                            LineMark(x: .value(String(localized: "Date"), t.date), y: .value("DPS", d)).foregroundStyle(Metric.efficiency.color)
                                            PointMark(x: .value(String(localized: "Date"), t.date), y: .value("DPS", d)).foregroundStyle(Metric.efficiency.color)
                                                .accessibilityLabel(t.label).accessibilityValue(String(localized: "\(String(format: "%.2f", d)) metres per stroke"))
                                        }
                                    }
                                }
                                .chartYScale(domain: .automatic(includesZero: false))
                                .chartYAxis {
                                    AxisMarks { value in
                                        AxisGridLine()
                                        AxisValueLabel { if let d = value.as(Double.self) { Text(String(format: "%.2f", d)) } }
                                    }
                                }
                                .frame(height: 160)
                            }
                            .card()
                        }
                    }
                    if trend.contains(where: { $0.tss != nil }) {
                        titled(String(localized: "Training load")) {
                            VStack(alignment: .leading, spacing: 16) {
                                StatTile(title: String(localized: "Latest session"), value: tss.map(whole) ?? "–", metric: .load)
                                Chart {
                                    ForEach(trend) { t in
                                        if let tss = t.tss {
                                            BarMark(x: .value(String(localized: "Date"), t.date), y: .value("TSS", tss)).foregroundStyle(Metric.load.color)
                                                .accessibilityLabel(t.label).accessibilityValue(String(localized: "Training load \(whole(tss))"))
                                        }
                                    }
                                }
                                .frame(height: 200)
                            }
                            .card()
                        }
                    }
                    titled(String(localized: "Personal records")) {
                        VStack(spacing: 0) {
                            if records.isEmpty { Text("No records yet").foregroundStyle(.secondary) }
                            ForEach(records.indices, id: \.self) { i in
                                if i > 0 { Divider().padding(.vertical, 12) }
                                let r = records[i]
                                let gear = r.equipment.map(equipmentName).joined(separator: ", ")
                                NavigationLink { SessionDetailView(id: r.sessionId) } label: {
                                    HStack(spacing: 12) {
                                        IconBadge(systemName: "trophy.fill", metric: .time)
                                        Text("\(r.distanceM) m \(r.stroke.name)\(gear.isEmpty ? "" : " · \(gear)")").font(.headline)
                                        Spacer()
                                        Text(swimTime(r.timeS))
                                            .font(.system(.title3, design: .rounded).weight(.semibold).monospacedDigit())
                                            .foregroundStyle(Metric.time.color)
                                        Image(systemName: "chevron.right").font(.footnote.bold()).foregroundStyle(.tertiary)
                                    }
                                }
                                .tint(.primary) // the app-wide distance tint would otherwise colour the label
                            }
                        }
                        .card()
                    }
                }
                .padding()
            }
            .background(Color(.systemBackground))
            .overlay {
                if trend.isEmpty {
                    ContentUnavailableView("No trends yet", systemImage: "chart.xyaxis.line",
                                           description: Text("Swim with Swimlie on your Apple Watch to see your trends here."))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(Color(.systemBackground))
                }
            }
            .navigationTitle("Trends")
            .onAppear {
                reload()
                #if DEBUG
                // simctl cannot scroll, so -demo trends-hr jumps to the heart rate section.
                if UserDefaults.standard.string(forKey: "demo") == "trends-hr" {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { proxy.scrollTo("hr", anchor: .top) }
                }
                #endif
            }
            .onReceive(NotificationCenter.default.publisher(for: Sync.changed)) { _ in reload() }
            .errorAlert($error)
        }
    }

    private func reload() {
        do {
            let css = try Sync.store?.latestCssSPer100m()
            let stored = try Sync.store?.listSessions() ?? []
            weeks = weeklyTotals(sessions: stored, utcOffsetS: Int32(TimeZone.current.secondsFromGMT()))
            trend = stored.sorted { $0.session.startedAtMs < $1.session.startedAtMs }.map { s in
                let sum = sessionSummary(session: s.session, cssSPer100m: css)
                return Trend(id: s.id, date: Date(timeIntervalSince1970: Double(s.session.startedAtMs) / 1000),
                             paceS: sum.paceSPer100m,
                             avgBpm: sum.avgBpm, maxBpm: sum.maxBpm, swolfAvg: sum.swolfAvg,
                             distancePerStrokeM: sum.distancePerStrokeM, tss: sum.tss)
            }
            records = try Sync.store?.personalRecords() ?? []
        } catch { self.error = error.localizedDescription }
    }
}

private func weekDate(_ w: WeekTotal) -> Date { Date(timeIntervalSince1970: Double(w.weekStartMs) / 1000) }

/// "This week" when the week contains today, else its start date.
private func weekLabel(_ w: WeekTotal) -> String {
    let start = weekDate(w)
    let end = Calendar.current.date(byAdding: .day, value: 7, to: start) ?? start
    return start <= .now && .now < end ? String(localized: "This week") : String(localized: "Week of \(start.formatted(.dateTime.day().month(.abbreviated)))")
}

private struct Trend: Identifiable {
    let id: String
    let date: Date
    let paceS: Double? // nil for sessions without distance
    let avgBpm: Double?
    let maxBpm: Double?
    let swolfAvg: Double?
    let distancePerStrokeM: Double?
    let tss: Double?

    var label: String { date.formatted(date: .long, time: .omitted) }
}
