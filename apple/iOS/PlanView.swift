import SwimlieCore
import SwiftUI

/// The sent id of every workout in plan order, so a swim links back to its plan workout.
func planWorkoutIds(_ plan: StoredPlan) -> [String] {
    plan.plan.weeks.indices.flatMap { w in
        plan.plan.weeks[w].workouts.indices.map { planWorkoutId(planId: plan.id, week: UInt32(w), index: UInt32($0)) }
    }
}

/// A plan's weeks as sections. The next swim not yet done is tinted and can be sent to the watch.
struct PlanView: View {
    let plan: StoredPlan
    @ObservedObject var library: Library
    @SwiftUI.State private var onWatch: String? // id of the workout last sent to the watch

    private func send(_ id: String, _ workout: Workout) {
        do { try Sync.sendWorkout(StoredWorkout(id: id, createdAtMs: 0, updatedAtMs: 0, workout: workout)); onWatch = id } catch { library.error = error.localizedDescription }
    }

    var body: some View {
        let next = planWorkoutIds(plan).first { !library.swum.contains($0) }
        List {
            ForEach(plan.plan.weeks.indices, id: \.self) { w in
                let week = plan.plan.weeks[w]
                Section {
                    ForEach(week.workouts.indices, id: \.self) { i in
                        let workout = week.workouts[i]
                        let id = planWorkoutId(planId: plan.id, week: UInt32(w), index: UInt32(i))
                        NavigationLink {
                            WorkoutDetail(workout: workout, css: library.css)
                        } label: {
                            WorkoutRow(workout: workout, css: library.css, onWatch: onWatch == id, done: library.swum.contains(id), send: { send(id, workout) })
                        }
                            .listRowBackground(id == next ? Metric.distance.color.opacity(0.15) : Color(.secondarySystemGroupedBackground))
                    }
                } header: {
                    Text(week.focus.map { String(localized: "Week \(w + 1) · \($0)") } ?? String(localized: "Week \(w + 1)"))
                }
            }
        }
        .navigationTitle(plan.plan.name)
        .onAppear { onWatch = Sync.sentWorkoutId }
        .errorAlert($library.error)
    }
}
