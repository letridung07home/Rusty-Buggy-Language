# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-08-25

### Added

- Added `-V` and `--version` output for the command-line evaluator.

## [0.3.3] - 2026-08-25

### Added

- Added `-h` and `--help` output for the command-line evaluator.

## [0.3.2] - 2026-08-25

### Changed

- Decoupled the Rust action revision from compiler selection and pinned the
  action while keeping stable and MSRV toolchains explicit.
- Split GitHub Actions Dependabot updates from grouped Cargo updates for
  independent review.
- Upgraded the release action to the supported Node 24 v3.0.2 runtime and
  pinned it to its full commit SHA.

## [0.3.1] - 2026-08-25

### Added

- Added standard-library integration tests that exercise the compiled CLI's
  success, error-output, and failure-status behavior.
- Added weekly grouped Dependabot updates for GitHub Actions and Cargo
  dependencies.

### Changed

- Declared Rust 1.70 as the minimum supported Rust version and split CI into
  stable quality checks and Rust 1.70 compatibility checks.
- Hardened release validation to require matching numeric SemVer tags and
  nonempty changelog entries, with deterministic links to the prior release.
- Updated contributor guidance to document stable-toolchain and MSRV coverage.

## [0.3.0] - 2026-08-25

### Added

- Added prefix unary negation with right-to-left associativity and precedence
  tighter than multiplication and division.
- Added support for the negated `i64::MIN` literal and coverage for negation,
  range errors, unary-plus rejection, and negation/division overflow.

## [0.2.0] - 2026-08-25

### Added

- Added a command-line integer expression evaluator with precedence,
  parentheses, checked signed 64-bit arithmetic, and clear errors.
- Added unit coverage for successful expressions and all documented failure
  classes.

## [0.1.1] - 2026-08-25

### Added

- Added contributor guidance for repository setup, validation, and releases.

## [0.1.0] - 2026-08-25

### Added

- Initialized the Rust package and executable entry point.
- Added GitHub Actions workflows for validation and tag-based releases.

### Changed

- Dual-licensed the project under MIT OR Apache-2.0.
