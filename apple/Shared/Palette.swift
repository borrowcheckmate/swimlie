import SwiftUI

extension Color {
    static let accent = Color(red: 0.3, green: 0.68, blue: 1)
}

/// One colour per kind of number, so the same metric looks the same on every screen.
enum Metric {
    case time, distance, pace, heart, strokes, efficiency, load

    var color: Color {
        switch self {
        case .time: Color(red: 1, green: 0.84, blue: 0.04)
        case .distance: .accent
        case .pace: Color(red: 0.39, green: 0.9, blue: 0.75)
        case .heart: Color(red: 1, green: 0.22, blue: 0.37)
        case .strokes: .orange
        case .efficiency: Color(red: 0.6, green: 0.95, blue: 0.3)
        case .load: Color(red: 0.75, green: 0.45, blue: 1)
        }
    }
}

/// Apple's heart rate zone colors, zone 1 first.
let zoneColors: [Color] = [.accent, .green, .yellow, .orange, .red]
