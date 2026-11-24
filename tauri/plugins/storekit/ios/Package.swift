// swift-tools-version:5.5

import PackageDescription

let package = Package(
  name: "tauri-plugin-storekit",
  platforms: [
    .macOS(.v10_13),
    .iOS(.v15),
  ],
  products: [
    .library(
      name: "tauri-plugin-storekit",
      type: .static,
      targets: ["tauri-plugin-storekit"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-storekit",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)
