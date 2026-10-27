// swift-tools-version: 5.9
// Swift wrapper around the Rust core. Run cargo xtask xcframework first: it generates
// SwimlieCoreFFI.xcframework and Sources/SwimlieCore/swimlie_core.swift.
import PackageDescription

let package = Package(
    name: "SwimlieCore",
    platforms: [.iOS(.v17), .watchOS(.v10)],
    products: [.library(name: "SwimlieCore", targets: ["SwimlieCore"])],
    targets: [
        .binaryTarget(name: "SwimlieCoreFFI", path: "SwimlieCoreFFI.xcframework"),
        .target(name: "SwimlieCore", dependencies: ["SwimlieCoreFFI"]),
    ]
)
