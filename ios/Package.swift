// swift-tools-version:6.1.0
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "live-activity",
    platforms: [
        .iOS(.v15),
    ],
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "live-activity",
            type: .static,
            targets: ["live-activity"]),
    ],
    dependencies: [
        .package(name: "Tauri", path: "../.tauri/tauri-api")
    ],
    targets: [
        // Targets are the basic building blocks of a package. A target can define a module or a test suite.
        // Targets can depend on other targets in this package, and on products in packages this package depends on.
        .target(
            name: "live-activity",
            dependencies: [
                .byName(name: "Tauri")
            ],
            path: "Sources",
            linkerSettings: [
                .linkedFramework("ActivityKit"),
                .linkedFramework("UserNotifications"),
                .linkedFramework("SwiftUI"),
                .linkedFramework("Foundation")
            ]
        )
    ]
)
