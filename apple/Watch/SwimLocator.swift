import CoreLocation
import SwimlieCore

/// One position at the start of a swim, to name the pool. Only when "saveLocation" is on; errors and no fix mean no location.
@MainActor final class SwimLocator: NSObject, CLLocationManagerDelegate {
    private(set) var location: Location?
    private let manager = CLLocationManager()

    override init() {
        super.init()
        manager.delegate = self
        manager.desiredAccuracy = kCLLocationAccuracyHundredMeters
    }

    /// Asks for one fix at the start of a swim; a fix from an earlier swim is dropped first.
    func request() {
        location = nil
        guard UserDefaults.standard.bool(forKey: "saveLocation"), [.authorizedWhenInUse, .authorizedAlways].contains(manager.authorizationStatus) else { return }
        manager.requestLocation()
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        let fixes = locations.filter { $0.horizontalAccuracy >= 0 }.map { Location(lat: $0.coordinate.latitude, lon: $0.coordinate.longitude, accuracyM: $0.horizontalAccuracy) }
        Task { @MainActor in if self.location == nil, let fix = fixes.first { self.location = fix } }
    }

    /// Required by requestLocation (else it raises); the swim just has no location.
    nonisolated func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {}
}
