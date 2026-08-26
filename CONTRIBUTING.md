# Contributing

Thanks for contributing to Rusty Buggy Language. This guide covers the local
setup, required checks, and release process.

## Repository setup

1. Install the stable Rust toolchain with `rustup`.
2. Clone the repository and change into its directory:

   ```bash
   git clone https://github.com/letridung07home/Rusty-Buggy-Language.git
   cd Rusty-Buggy-Language
   ```

3. Create a branch for your change, make the smallest focused edit, and update
   documentation or tests when the behavior changes.

## Expected Rust checks

Before opening a pull request, the project expects these commands to pass:

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo test --all-targets --all-features
cargo test --doc --all-features
cargo test --release --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

The GitHub Actions workflow runs these stable-toolchain quality checks on
pushes to `main`, on pull requests, and on demand from the Actions tab
(`workflow_dispatch`). It also compiles and tests the package
with Rust 1.70, the project's minimum supported Rust version (MSRV), so new
changes must remain compatible with both stable Rust and the MSRV. A separate
semver job compares the public library API against the latest release tag
with `cargo-semver-checks`, failing on any change that would break the v1
series backward-compatibility commitment. In the maintainer environment, Rust
checks, builds, and tests must run through GitHub Actions rather than
locally. Use `gh run` to monitor the workflow and inspect its logs.

## Release process

For a versioned release:

1. Update the version in `Cargo.toml`.
2. Move the relevant `Unreleased` entries into a dated
   `## [X.Y.Z] - YYYY-MM-DD` section in `CHANGELOG.md`.
3. Commit the release preparation and push it to `main`.
4. Wait for the GitHub Actions CI workflow to succeed.
5. Create and push an annotated tag:

   ```bash
   git tag -a vX.Y.Z -m "Release vX.Y.Z"
   git push origin vX.Y.Z
   ```

6. Confirm with `gh` that the tag-triggered release workflow creates
   `RBL vX.Y.Z`, uses the matching changelog section as its description, and
   attaches the prebuilt Linux, macOS, and Windows binaries it built from the
   tag.

Do not add a co-author to release commit messages.
