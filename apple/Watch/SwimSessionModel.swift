import Foundation
import Observation
import SwimlieCore

/// What the watch UI needs from a live swim. `SwimSession` (HealthKit) and `FakeSwimSession` (simulator lap feed) implement it.
@MainActor protocol SwimSessionModel: AnyObject, Observable {
    /// Workout being swum; nil before `start`.
    var workout: Workout? { get }
    /// Latest engine state; nil before the first tick.
    var state: State? { get }
    /// The swim as saved; set when `end` finishes.
    var saved: Session? { get }
    /// `end` has run: show the summary (`saved` stays nil when the swim never started).
    var ended: Bool { get }
    /// `end` has started and the swim is being saved: the live screen shows it and ignores the crown.
    var saving: Bool { get }
    /// Last start/save failure, for display.
    var error: String? { get }
    /// Latest heart rate in bpm; nil until the first sample.
    var heartRate: Double? { get }
    /// Swimmer's age from Health, for HR zones.
    var ageYears: UInt32? { get }
    func start(stored: StoredWorkout)
    /// Manual "rep done" / "skip rest".
    func manualNext()
    /// Undo the latest manual next; false when there is nothing to undo.
    @discardableResult func manualBack() -> Bool
    /// The HealthKit session is paused (crown + side button, or the controls page).
    var paused: Bool { get }
    /// When `state` was computed, so a view can keep its clocks running between ticks.
    var stateDate: Date { get }
    func togglePause()
    func end()
}
