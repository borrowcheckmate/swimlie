import SwimlieCore
import SwiftUI

struct DetailView: View {
    let store: Store
    let workout: Workout
    let onStart: () -> Void
    @Environment(\.isLuminanceReduced) private var wristDown

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 4) {
                Text(workout.name).font(.headline).foregroundStyle(Color.accent)
                Text(metres(totalDistanceM(workout: workout))).font(.system(size: 34, weight: .bold, design: .rounded))
                    .minimumScaleFactor(0.6).lineLimit(1).foregroundStyle(Metric.distance.color).opacity(wristDown ? 0.6 : 1)
                let css = try? store.latestCssSPer100m()
                Text("\(mmss(estimatedDurationS(workout: workout, cssSPer100m: css))) · \(workout.poolLengthM)m pool")
                    .font(.footnote).foregroundStyle(.secondary)
                ForEach(Array(workout.blocks.enumerated()), id: \.offset) { _, b in
                    HStack {
                        Text(b.name).font(.subheadline.bold()).foregroundStyle(Color.accent)
                        Spacer()
                        if b.repeat > 1 {
                            Label("\(b.repeat)×", systemImage: "repeat").font(.subheadline.bold())
                                .padding(.horizontal, 8).padding(.vertical, 2).background(Color.accent.opacity(0.25), in: Capsule())
                        }
                    }.padding(.top, 6)
                    Divider()
                    // A repeated block: its sets sit behind an accent bar, so the group reads as one round.
                    let rounds = b.repeat > 1
                    VStack(alignment: .leading, spacing: 4) {
                    ForEach(Array(b.sets.enumerated()), id: \.offset) { _, s in
                        let title = HStack(spacing: 6) {
                            Text(s.title).font(.system(.body, design: .rounded).bold()).lineLimit(1).minimumScaleFactor(0.7)
                            EquipmentRow(equipment: s.equipment)
                        }
                        let labels = HStack(spacing: 8) {
                            if let r = s.rest { Label(mmss(Double(r.seconds)), systemImage: "timer") }
                            if let g = s.goalS { Label(mmss(Double(g)), systemImage: "target") }
                        }.font(.footnote).fixedSize() // whole or wrapped: a label never breaks inside
                        ViewThatFits { // at large text the labels wrap under the title
                            HStack(spacing: 6) { title; Spacer(minLength: 0); labels }
                            VStack(alignment: .leading, spacing: 2) { title; labels }
                        }
                        DrillCue(set: s)
                    }
                    }
                    .padding(.leading, rounds ? 8 : 0)
                    .overlay(alignment: .leading) { if rounds { Capsule().fill(Color.accent).frame(width: 3) } }
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            Button(action: onStart) {
                Image(systemName: "play.fill").font(.title2).foregroundStyle(.white)
                    .frame(width: 56, height: 56).background(Color.green, in: Circle())
            }.buttonStyle(.plain).frame(maxWidth: .infinity)
        }
        .themed()
    }
}
