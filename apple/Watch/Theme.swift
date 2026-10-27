import SwimlieCore
import SwiftUI

/// Assigns `new` through `assign`. A new screen (phase, rep or pause changed) animates so the transition runs; the once-a-second ticks do not.
/// The write happens inside the closure: an inout would be written back after withAnimation returned.
@MainActor func publish(_ new: SwimState, after old: SwimState?, _ assign: (SwimState) -> Void) {
    let newScreen = old.map { $0.phase != new.phase || $0.pos != new.pos || ($0.restRemainingS == nil) != (new.restRemainingS == nil) } ?? true
    if newScreen { withAnimation(.snappy) { assign(new) } } else { assign(new) }
}

func mmss(_ s: Double) -> String {
    let t = Int(s.rounded())
    return String(format: "%d:%02d", t / 60, t % 60)
}

/// Signed seconds with a real minus sign: "−3.2 s" / "+1.4 s".
func signedSeconds(_ s: Double) -> String {
    let r = (s * 10).rounded() / 10
    return (r < 0 ? "−" : "+") + String(format: "%.1f", abs(r)) + " s"
}

func pace(_ s: Double?) -> String { s.map { mmss($0) + " /100m" } ?? "-" }

func metres(_ m: UInt32) -> String { m.formatted(.number.grouping(.automatic)) + " m" }

/// Big rounded digits for times and distances: one look on the live and recorder screens.
func digits(_ size: CGFloat) -> Font { .system(size: size, weight: .bold, design: .rounded).monospacedDigit() }

extension Stroke {
    var short: String {
        switch self {
        case .any: String(localized: "ANY")
        case .freestyle: "FR"
        case .backstroke: "BK"
        case .breaststroke: "BR"
        case .butterfly: String(localized: "FLY")
        case .mixed: "IM"
        case .kickboard: String(localized: "KICK")
        }
    }
}

extension Intensity {
    var color: Color {
        switch self {
        case .easy: .accent
        case .moderate: .green
        case .threshold: .yellow
        case .hard: .orange
        case .sprint: .red
        }
    }
}

extension Verdict {
    var color: Color {
        switch self {
        case .fastest: .purple
        case .faster: .green
        case .slower: .orange
        }
    }
}

/// Full-screen fill colors: lighter and more saturated than the system ones, to show through water and goggles.
extension Color {
    static let waterGreen = Color(red: 0.3, green: 1, blue: 0.4)
    static let waterOrange = Color(red: 1, green: 0.6, blue: 0)
    static let waterBlue = Color(red: 0.25, green: 0.8, blue: 1)
}

extension Equipment {
    /// Asset catalog image name (template-rendered, so foregroundStyle tints it).
    var asset: String {
        switch self {
        case .fins: "fins"
        case .paddles: "paddles"
        case .pullBuoy: "pull_buoy"
        case .kickboard: "kickboard"
        case .snorkel: "snorkel"
        }
    }
    var label: String {
        switch self {
        case .fins: String(localized: "Fins")
        case .paddles: String(localized: "Paddles")
        case .pullBuoy: String(localized: "Pull buoy")
        case .kickboard: String(localized: "Kickboard")
        case .snorkel: String(localized: "Snorkel")
        }
    }
}

/// Heart rate zones the Apple way, kept small: five blocks in the zone colors, the current one wide with its number.
/// A white line under the target zones: a wide block right of the line means too much effort.
struct ZoneBar: View {
    let zone: UInt8?
    let target: [UInt8]
    var body: some View {
        HStack(alignment: .top, spacing: 2) {
            ForEach(1...5, id: \.self) { z in
                let on = zone == UInt8(z)
                VStack(spacing: 2) {
                    RoundedRectangle(cornerRadius: 2)
                        .fill(zoneColors[z - 1].opacity(on ? 1 : 0.4))
                        .frame(width: on ? 14 : 4, height: on ? 12 : 4)
                        .overlay { if on { Text("\(z)").font(.system(size: 10, weight: .heavy, design: .rounded)).blendMode(.destinationOut) } } // a cutout: readable over a fill too
                        .compositingGroup()
                        .frame(height: 12)
                    Capsule().fill(target.contains(UInt8(z)) ? .white : .clear).frame(width: on ? 14 : 4, height: 1.5)
                }
            }
        }
        .animation(.snappy, value: zone)
        .accessibilityElement().accessibilityLabel(zone.map { String(localized: "Zone \($0)") } ?? String(localized: "No zone"))
    }
}

/// Equipment icons for a set; shows nothing when the set has none.
struct EquipmentRow: View {
    let equipment: [Equipment]
    var body: some View {
        if !equipment.isEmpty {
            HStack(spacing: 6) {
                ForEach(equipment, id: \.self) { e in Image(e.asset).resizable().scaledToFit().frame(width: 14, height: 14).accessibilityLabel(e.label) }
            }
            .foregroundStyle(.white.opacity(0.85))
        }
    }
}

/// The set's drill (catalogue exercise) as "Name: cue", one or two short lines; nothing without a known drill.
struct DrillCue: View {
    let set: WorkoutSet
    @Environment(\.isLuminanceReduced) private var wristDown

    var body: some View {
        if let e = set.drill.flatMap({ SwimlieCore.exerciseIn(id: $0, lang: Bundle.main.preferredLocalizations.first == "nl" ? "nl" : "en") }) {
            (Text(e.name + ": ").bold() + Text(e.cue))
                .font(.footnote).foregroundStyle(.secondary).opacity(wristDown ? 0.6 : 1)
                .multilineTextAlignment(.leading).lineLimit(2).minimumScaleFactor(0.8)
        }
    }
}

extension WorkoutSet {
    var title: String { "\(reps)×\(distanceM) \(stroke.short)" }
}

extension Workout {
    /// The set at `pos`; nil when out of range.
    func set(at pos: Position) -> WorkoutSet? {
        guard Int(pos.block) < blocks.count else { return nil }
        let sets = blocks[Int(pos.block)].sets
        return Int(pos.set) < sets.count ? sets[Int(pos.set)] : nil
    }
}

extension View {
    /// Black screen with the shared dark look (OLED: black pixels are off).
    func themed() -> some View { background(Color.black.ignoresSafeArea()).foregroundStyle(.white) }
}
typealias SwimState = SwimlieCore.State
