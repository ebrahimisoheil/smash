// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "SmashBar",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "SmashBar",
            path: "Sources/SmashBar",
            resources: [.process("Resources")]
        )
    ]
)
