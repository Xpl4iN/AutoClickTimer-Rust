# Versioning

Increment an app version for every change in this repository before finishing the changeset.

- For Rust desktop, CLI, MCP, documentation, or repository changes, increment `Cargo.toml` package version and update `Cargo.lock`.
- For Flutter mobile changes, increment both the version and build number in `mobile/pubspec.yaml` and keep `mobile/lib/services/update_service.dart` at the same release version.
- For changes affecting both apps, increment both versions.
- For user-visible changes, add a brief entry to the relevant `README.md` What's New section.

Verify that the built Rust executable's `version` command reports the version in `Cargo.toml`.

When pushing a new version, publish a GitHub release for that version with the built Windows executable. Include the Android APK when the mobile app changes.
