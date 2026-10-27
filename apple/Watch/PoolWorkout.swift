import WatchKit
import HealthKit
import SwimlieCore

/// HealthKit setup and lap-event parsing shared by Recorder (spike) and SwimSession.
enum PoolWorkout {
    /// Water lock during a swim; on unless switched off in the menu (handy when testing at a desk).
    static func enableWaterLockIfWanted() {
        if UserDefaults.standard.object(forKey: "waterLock") as? Bool ?? true { WKInterfaceDevice.current().enableWaterLock() }
    }

    /// Asks for auth, then builds the pool-swim session and live builder. Caller sets delegates and starts them.
    static func make(store: HKHealthStore, poolLengthM: Double) async throws -> (HKWorkoutSession, HKLiveWorkoutBuilder) {
        let read: Set<HKObjectType> = [
            HKQuantityType(.distanceSwimming), HKQuantityType(.swimmingStrokeCount),
            HKQuantityType(.activeEnergyBurned), HKQuantityType(.heartRate), HKCharacteristicType(.dateOfBirth),
        ]
        // The live builder saves the samples it collects, so it needs share access to them too.
        let share: Set<HKSampleType> = [
            HKObjectType.workoutType(), HKQuantityType(.distanceSwimming),
            HKQuantityType(.swimmingStrokeCount), HKQuantityType(.activeEnergyBurned),
        ]
        try await store.requestAuthorization(toShare: share, read: read)
        guard store.authorizationStatus(for: HKObjectType.workoutType()) == .sharingAuthorized else {
            throw HKError(.errorAuthorizationDenied)
        }

        let config = HKWorkoutConfiguration()
        config.activityType = .swimming
        config.swimmingLocationType = .pool
        config.lapLength = HKQuantity(unit: .meter(), doubleValue: poolLengthM)

        let session = try HKWorkoutSession(healthStore: store, configuration: config)
        let builder = session.associatedWorkoutBuilder()
        builder.dataSource = HKLiveWorkoutDataSource(healthStore: store, workoutConfiguration: config)
        return (session, builder)
    }

    /// Identity of an event; workoutEvents is cumulative, so this dedupes repeats.
    static func key(_ e: HKWorkoutEvent) -> String {
        "\(e.type.rawValue)|\(e.dateInterval.start.timeIntervalSince1970)|\(e.dateInterval.end.timeIntervalSince1970)"
    }

    static func strokeStyle(_ e: HKWorkoutEvent) -> HKSwimmingStrokeStyle? {
        (e.metadata?[HKMetadataKeySwimmingStrokeStyle] as? Int).flatMap(HKSwimmingStrokeStyle.init(rawValue:))
    }

    static func stroke(_ s: HKSwimmingStrokeStyle?) -> Stroke {
        switch s {
        case .freestyle: .freestyle
        case .backstroke: .backstroke
        case .breaststroke: .breaststroke
        case .butterfly: .butterfly
        case .kickboard: .kickboard
        default: .mixed // unknown/mixed/missing: neutral for the engine's stroke check
        }
    }
}
