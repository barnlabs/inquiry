// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "InquiryMac",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "Inquiry", targets: ["Inquiry"])],
    targets: [
        .executableTarget(
            name: "Inquiry",
            path: "macos/Sources/Inquiry"
        ),
        .testTarget(
            name: "InquiryTests",
            dependencies: ["Inquiry"],
            path: "macos/Tests/InquiryTests"
        )
    ]
)

