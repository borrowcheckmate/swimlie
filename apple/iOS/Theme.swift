import SwimlieCore
import SwiftUI

struct StatTile: View {
    let title: String
    let value: String
    var unit: String?
    let metric: Metric

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(String(localized: String.LocalizationValue(title))).font(.subheadline).foregroundStyle(.secondary)
            HStack(alignment: .firstTextBaseline, spacing: 3) {
                Text(value).font(.system(.title, design: .rounded).weight(.semibold).monospacedDigit())
                if let unit { Text(unit).font(.system(.title3, design: .rounded).weight(.semibold)) }
            }
            .foregroundStyle(metric.color)
        }
    }
}

/// Metric-coloured icon in a tinted circle, for list rows.
struct IconBadge: View {
    let systemName: String
    let metric: Metric

    var body: some View {
        Image(systemName: systemName)
            .font(.title3)
            .foregroundStyle(metric.color)
            .frame(width: 44, height: 44)
            .background(metric.color.opacity(0.2), in: Circle())
    }
}

extension View {
    func card() -> some View {
        padding(16)
            .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 20, style: .continuous))
    }
}

extension View {
    /// Shows a failed load, import or delete as an alert; nil means no error.
    func errorAlert(_ error: Binding<String?>) -> some View {
        alert("Error", isPresented: Binding(get: { error.wrappedValue != nil }, set: { if !$0 { error.wrappedValue = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(error.wrappedValue ?? "")
        }
    }
}

/// Fitness-style section title, sitting above its card.
func titled<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
    VStack(alignment: .leading, spacing: 12) {
        Text(String(localized: String.LocalizationValue(title))).font(.title3.bold())
        content()
    }
}

extension Stroke {
    /// The one name shown for a stroke on every screen.
    var name: String {
        switch self {
        case .any: String(localized: "Any")
        case .freestyle: String(localized: "Freestyle")
        case .backstroke: String(localized: "Backstroke")
        case .breaststroke: String(localized: "Breaststroke")
        case .butterfly: String(localized: "Butterfly")
        case .mixed: String(localized: "Mixed stroke")
        case .kickboard: String(localized: "Kickboard")
        }
    }
}

/// Whole number for display.
func whole(_ x: Double) -> String { "\(Int(x.rounded()))" }

/// m:ss, or h:mm:ss from an hour.
func clock(_ seconds: Double) -> String {
    let t = Int(seconds.rounded())
    let h = t / 3600, m = (t % 3600) / 60, s = t % 60
    return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
}

/// "1 minute 45 seconds per 100 metres", for VoiceOver.
func spokenPace(_ seconds: Double) -> String {
    let t = Int(seconds.rounded())
    let m = t / 60, s = t % 60
    return m > 0 ? String(localized: "\(m) min \(s) s per 100 metres") : String(localized: "\(s) s per 100 metres")
}

/// Pace axis tick step in seconds, so a wide pace range doesn't crowd the labels.
func paceStride(_ paces: [Double]) -> Double {
    let span = (paces.max() ?? 0) - (paces.min() ?? 0)
    return span <= 20 ? 5 : span <= 60 ? 15 : 30
}
