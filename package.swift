// swift-tools-version: 5.10
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let binaryTarget: Target = .binaryTarget(
    name: "LniCoreRS",
    // IMPORTANT: Swift packages importing this locally will not be able to
    // import the rust core unless you use a relative path.
    // This ONLY works for local development. For a larger scale usage example, see https://github.com/stadiamaps/ferrostar.
    // When you release a public package, you will need to build a release XCFramework,
    // upload it somewhere (usually with your release), and update Package.swift.
    // This will probably be the subject of a future blog.
    // Again, see Ferrostar for a more complex example, including more advanced GitHub actions.
    path: "./lni/target/ios/liblni-rs.xcframework"
)

let package = Package(
    name: "Lni",
    platforms: [
        .iOS(.v16),
    ],
    products: [
        // Products define the executables and libraries a package produces, making them visible to other packages.
        .library(
            name: "Lni",
            targets: ["Lni"]
        ),
    ],
    targets: [
        binaryTarget,
        .target(
            name: "Lni",
            dependencies: [.target(name: "UniFFI")],
            path: "apple/Sources/Lni"
        ),
        .target(
            name: "UniFFI",
            dependencies: [.target(name: "LniCoreRS")],
            path: "apple/Sources/UniFFI"
        ),
        .testTarget(
            name: "LniTests",
            dependencies: ["Lni"],
            path: "apple/Tests/LniTests"
        ),
    ]
)