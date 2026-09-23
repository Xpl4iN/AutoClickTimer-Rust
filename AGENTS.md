# Versioning

Increment an app version for every change in this repository before finishing the changeset.

- For Rust desktop, CLI, MCP, documentation, or repository changes, increment `Cargo.toml` package version and update `Cargo.lock`.
- For Flutter mobile changes, increment both the version and build number in `mobile/pubspec.yaml`.
- For changes affecting both apps, increment both versions.
- For user-visible changes, add a brief entry to the relevant `README.md` What's New section.

Verify that the built Rust executable's `version` command reports the version in `Cargo.toml`.
