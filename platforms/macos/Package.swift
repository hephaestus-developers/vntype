// swift-tools-version: 6.0
// Cấu hình gói Swift Package Manager cho ứng dụng VNType InputMethodKit trên macOS.

import PackageDescription

let package = Package(
    name: "VNTypeMacOS",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(name: "VNTypeIM", targets: ["VNTypeIM"])
    ],
    targets: [
        .executableTarget(
            name: "VNTypeIM",
            path: "Sources",
            exclude: ["Bridging-Header.h"],
            swiftSettings: [
                .unsafeFlags(["-import-objc-header", "Sources/Bridging-Header.h"])
            ],
            linkerSettings: [
                .unsafeFlags(["-L../../target/release", "-lvntype_ffi"]),
                .linkedFramework("Cocoa"),
                .linkedFramework("InputMethodKit"),
                .linkedFramework("Carbon")
            ]
        ),
        .testTarget(
            name: "MacAdapterTests",
            dependencies: ["VNTypeIM"],
            path: "Tests",
            swiftSettings: [
                .unsafeFlags(["-import-objc-header", "Sources/Bridging-Header.h"])
            ]
        )
    ]
)
