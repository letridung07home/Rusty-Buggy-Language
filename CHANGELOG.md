# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added an interactive REPL mode via `rusty-buggy-language --repl` that reads
  one program per line from standard input and prints each result, reusing
  the exact CLI evaluator. A REPL that sees no programs exits unsuccessfully;
  otherwise errors are printed and the loop continues.
- Added a dependency-free property-based test that evaluates generated
  programs against an independent reference model covering arithmetic
  operators, nesting, and sequential immutable `let` bindings.
- Added a dependency-free fuzzing smoke harness over the public `evaluate`
  entry point with random and edge-case inputs, plus a bounded-time CI job
  that runs it with a hard timeout.
- Added a short introduction and reference in `docs/tutorial.md`, and an
  `examples/` directory of runnable programs.

### Changed

- The GitHub Actions release workflow now builds and attaches prebuilt
  `x86_64` binaries for Linux, macOS, and Windows to each GitHub release.

## [1.3.0] - 2026-08-25

### Added

- Added the modulo operator `%` with the same checked semantics as `/`: the
  remainder of truncated division. Modulo by zero reports `division by zero`,
  and `-9223372036854775808 % -1` reports `integer remainder overflow`.

## [1.2.0] - 2026-08-25

### Added

- Added `//` line comments and `/* */` block comments. Comments are stripped
  by the lexer and never affect evaluation; an unterminated block comment
  reports `unterminated block comment`.

## [1.1.0] - 2026-08-25

### Added

- Added source positions (line and column) to evaluation and syntax errors,
  tracked through the lexer and parser. Error messages are unchanged; the
  library exposes the position through `Error::position()`, and the CLI
  reports it with the opt-in `--positions` flag.
- Added a parser nesting-depth limit that rejects programs nested more deeply
  than 256 levels with `program too deeply nested` instead of overflowing the
  stack on adversarial input.
- Added a configurable input-size limit. The library gains
  `evaluate_with_limits(&str, &Limits)` alongside the unchanged `evaluate`,
  and the CLI gains an `--input-limit <bytes>` flag. Programs longer than the
  limit are rejected before parsing.

### Changed

- Restructured the roadmap into the v1.x series with concrete development
  goals (evaluator hardening, agent-friendly additions, and distribution)
  and the completed v1.0.0 milestone recorded separately.

## [1.0.0] - 2026-08-25

### Changed

- Declared the core language, public `evaluate` library API, and CLI source
  modes stable for the v1.0 contract.

## [0.9.0] - 2026-08-25

### Added

- Added a v1.0 roadmap defining the stable core-language scope, release
  requirements, and explicitly deferred features.

## [0.8.0] - 2026-08-25

### Added

- Added CLI support for reading complete UTF-8 programs from files with `-f`
  or `--file`, and from standard input with `--stdin`.

### Changed

- Expanded CLI help, documentation, and validation for mutually exclusive
  inline, file, and standard-input source modes while preserving the public
  `evaluate(&str) -> Result<i64, Error>` API.

## [0.7.0] - 2026-08-25

### Changed

- Converted the CLI-only crate into a reusable library with a public
  `evaluate` API and a thin CLI adapter while preserving language behavior,
  error messages, and command-line behavior.

## [0.6.0] - 2026-08-25

### Added

- Added integer comparison operators (`<`, `<=`, `>`, `>=`, `==`, and `!=`)
  that evaluate to `1` or `0`.

## [0.5.2] - 2026-08-25

### Fixed

- Derived CLI version-output test expectations from Cargo package metadata so
  they remain aligned with future package version bumps.

## [0.5.1] - 2026-08-25

### Changed

- Restructured project documentation by moving the language reference to
  `docs/`, adding development and maintenance guidance, and streamlining the
  README.

## [0.5.0] - 2026-08-25

### Added

- Added immutable integer variables with `let` declarations and sequential
  program evaluation.

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
