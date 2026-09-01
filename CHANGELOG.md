# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.4.0] - 2026-09-02

### Added

- Added a `--json` CLI mode that prints one machine-readable JSON document on
  stdout instead of prose, so agents can consume output without parsing text:
  success is `{"ok":true,"value":<result>,"type":"integer"|"boolean"|"string"}`
  and failure is `{"ok":false,"error":"<message>"[,"line":L,"column":C]}` —
  the position fields appear when the error carries a source position (the
  same positions `--positions` exposes). A JSON error document still exits
  with a failure status. Without the flag, output is unchanged. Hand-rolled
  JSON escaping keeps the crate dependency-free (`"`, `\\`, `\n`, `\t`, `\r`,
  and `\u00XX` for other control characters; non-ASCII passes through as
  UTF-8). Works with all source modes (inline, `-f/--file`, `--stdin`).

## [2.3.0] - 2026-08-29

### Added

- Added five built-in functions: `len(s)` returns the number of Unicode
  scalar values in a string (characters, not UTF-8 bytes); `int_to_string(n)`
  returns an integer's decimal text; `string_to_int(s)` parses that text
  back — an optional leading `-` followed by one or more ASCII decimal
  digits, no whitespace or `+`, and a magnitude that fits in 64 bits —
  reporting the positioned error `invalid integer text: '<text>'` at the
  call for anything else; `bool_to_int(b)` maps `true`/`false` to `1`/`0`;
  and `int_to_bool(n)` maps `0` to `false` and any other integer to `true`.
- The ordering comparisons `<`, `<=`, `>`, and `>=` now also compare two
  strings lexicographically by Unicode scalar value; they previously
  accepted only two integers.
- Declaring a user function with a built-in's name (for example
  `fn len(s) = { 0 };`) is rejected with `duplicate function declaration:
  '<name>'`, so the builtins keep their fixed signatures. Builtins cannot
  be referenced as values (a bare `len` remains an undefined variable), and
  a builtin call consumes no call depth.
- Added `examples/stdlib.rbl` plus language-reference and tutorial sections
  covering the builtins and string ordering.

## [2.2.1] - 2026-08-29

### Fixed

- Fixed a stack overflow found by the nightly fuzz workflow (GitHub issue
  #13): a program combining deep recursion with a long prefix `-` chain (a
  recursive call guarded by 44 unary minuses) exhausted the stack, because
  stack cost grows as call depth times the per-call expression descent even
  when each alone stays within its own limit. The evaluator now also bounds
  total expression-evaluation depth at 2048 levels; exceeding it reports the
  existing `program too deeply nested` message with the expression's
  position. Regression tests and a fuzz corpus seed cover the crashing
  input.

## [2.2.0] - 2026-08-28

### Added

- Added function declarations `fn name(param, ...) = expression;` that later
  declarations and the final expression can call; bodies may be blocks so a
  function can declare its own `let` bindings. Parameters are immutable and
  scoped to the body, a function may call any earlier or later function,
  including itself (recursion) or another function that calls it back
  (mutual recursion).
- Added monomorphic type inference for functions, computed by fixed-point
  iteration over the small three-type lattice (`integer`, `boolean`,
  `string`): parameter and result types are inferred from each body and
  from every call site (including recursive and top-level calls) until
  they stabilize, so no explicit type annotations are needed.
- The type checker now reports positioned errors for calling an undefined
  function, passing the wrong number of arguments, and a call-site argument
  that does not match the inferred parameter type.
- Added `examples/functions.rbl` and `examples/recursion.rbl`, plus
  language-reference and tutorial sections covering declarations, calls,
  parameter scoping, and recursion.
- Added a dependency-free property-based test (`tests/property_functions.rs`)
  that checks generated `fn` programs (with an acyclic call graph) against an
  independent reference evaluator, plus fixed self- and mutual-recursion
  cases. Evaluation still bails out with a clear `call depth limit exceeded`
  error instead of overflowing the stack.

## [2.1.0] - 2026-08-28

### Added

- Added `true`/`false` boolean literals, prefix `!`, and short-circuiting
  `&&` and `||` operators.
- Added `"..."` string literals with `\n`, `\t`, `\\`, and `\"` escapes;
  `+` concatenates strings and `==`/`!=` compare values of the same type.
- Added `if`/`else` expressions with expression blocks
  (`{ let ...; expression }`) and lexical scoping, with shadowing allowed
  across scopes; `else` is required and both branches must have the same type.
- Added a static type checker that rejects ill-typed programs with a
  positioned type error before evaluation.
- Added tutorial sections and runnable examples for booleans, strings, and
  branching.

### Changed

- Comparisons now produce real booleans (`true`/`false`) instead of integer
  `1`/`0`, so a comparison result can no longer feed arithmetic directly:
  `let ready = 3 >= 2; ready * 10` is now a type error (write
  `if ready { 10 } else { 0 }` instead).
- The property-based reference model now generates and checks typed programs
  covering booleans, strings, and `if`/`else` alongside integer arithmetic.
- The parser nesting-depth limit is lowered to 128 levels: the richer
  expression grammar recurses through more frames per nesting level, so 256
  could overflow the stack on adversarial input instead of reporting
  `program too deeply nested`.

## [2.0.1] - 2026-08-26

### Changed

- The nightly fuzz workflow now runs its coverage-guided fuzzing in
  parallel across every core on the runner, so the same wall-clock budget
  buys roughly twice the executions and deeper coverage. It passes
  `-jobs`/`-workers` set to the runner's core count (instead of letting
  libFuzzer default to half the cores), with each worker sharing the corpus
  while receiving the full time budget.

## [2.0.0] - 2026-08-26

### Changed

- Breaking: `evaluate` and `evaluate_with_limits` now return
  `Result<Value, Error>` instead of `Result<i64, Error>`. The new public
  `Value` enum (`Int(i64)`, `Bool(bool)`, `String(String)`) is the typed
  evaluation result; v2.0 evaluation still produces only `Value::Int`, with
  the `Bool` and `String` variants defined now so later 2.x releases stay
  backward compatible with the v2.0.0 contract. `Error`,
  `SourcePosition`, and `Limits` are unchanged.
- The CLI prints each result through `Value`'s `Display` implementation, so
  v2.0 output is byte-for-byte identical to v1.x for every program;
  language semantics, error messages, and exit behavior are unchanged.
- The CI semver job now runs with `release-type: major` while the breaking
  v2 window is open, so the deliberate v1-to-v2 API change is reported
  rather than failing the check; the first v2 tag becomes the baseline at
  the v2.1 release.
## [1.6.3] - 2026-08-26

### Added

- The release workflow can now be triggered manually with
  `workflow_dispatch` by supplying an existing tag name, so a release
  whose asset upload failed can be re-created without retagging; the run
  builds and publishes exactly the requested tag.

### Changed

- CI runs for the same branch or pull request now cancel the previous
  in-flight run, so a newer push supersedes a stale one instead of
  consuming runner time.
- The release workflow now runs the release-mode test suite on the tag
  before building, so the tag is self-validating instead of racing the
  parallel CI run triggered by the tag push.

### Fixed

- The nightly fuzz workflow now clamps the manual `duration_minutes` input
  to a minimum of 1 minute in addition to the existing 180-minute maximum,
  so a `0` or negative value no longer means an unbounded
  `-max_total_time=0` fuzz run; the issue-filing body reports the same
  clamped duration.

## [1.6.2] - 2026-08-26

### Added

- The CI workflow can now be triggered manually with `workflow_dispatch`,
  so the full suite can be re-run on demand from the Actions tab.

### Changed

- CI now runs the full test suite in release mode, exercising the same
  build profile as the binaries shipped with each GitHub release.
- The line-coverage CI job no longer uploads reports to Codecov; it now
  prints the coverage summary in the job log and enforces the 80% line
  coverage threshold.
- GitHub releases now also ship arm64 Linux (`ubuntu-24.04-arm` native
  build) and x86_64 macOS (cross-compiled on the arm64 runner) binaries,
  alongside the existing x86_64 Linux, arm64 macOS, and x86_64 Windows
  builds.

### Fixed

- The nightly fuzz workflow now clamps the manual `duration_minutes`
  input to the documented 180-minute maximum, so an out-of-range value
  no longer runs until the job timeout kills it.
- The release workflow's changelog extraction now anchors on the exact
  `## [X.Y.Z]` header, so a version like 1.6.1 can no longer match a
  1.6.10 section.

## [1.6.1] - 2026-08-26

### Added

- Added `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]` crate-root
  lints to the library, so the crate remains free of `unsafe` and every
  public item stays documented (enforced by the documentation job).
- Added doctests for the `evaluate` and `evaluate_with_limits` entry points,
  run by CI with `cargo test --doc` on both stable Rust and Rust 1.70.
- Added a CI job that checks the public library API for backward
  compatibility against the latest release tag with `cargo-semver-checks`,
  machine-enforcing the v1 series no-breaking-changes commitment.

### Changed

- CI now runs doctests alongside the ordinary test suite.

## [1.6.0] - 2026-08-26

### Removed

- Dropped the roadmap goal to publish the crate to crates.io; distribution
  stays with the prebuilt GitHub-release binaries.

### Changed

- Closed out the v1.x roadmap: all development goals are delivered as of
  v1.6.0, and the roadmap now records candidate v2.0 directions.

## [1.5.1] - 2026-08-26

### Added

- Attached a `SHA256SUMS` file to each GitHub release so users can verify
  the downloaded binaries against their published hashes.

### Fixed

- Fixed the release workflow dropping the macOS binary and accidentally
  clobbering same-named builds: released binaries are now named by platform
  (`rusty-buggy-language-linux`, `rusty-buggy-language-macos`,
  `rusty-buggy-language-windows.exe`), so the Linux, macOS, and Windows
  artifacts never overwrite one another in the uploaded release.

## [1.5.0] - 2026-08-26

### Added

- Added a CI coverage job that measures line coverage with
  `cargo-llvm-cov` and fails when it drops below 80%, uploading the report
  to Codecov when a `CODECOV_TOKEN` secret is configured.
- Added an actionlint CI job that lints the GitHub Actions workflow files
  on every push and pull request, catching YAML, expression, and shell
  errors before they can reach a release tag.
- Added a coverage-guided `cargo-fuzz` (libFuzzer) fuzz target over the
  public `evaluate` entry point with a small committed seed corpus of
  arithmetic edge cases, plus a nightly GitHub Actions workflow that fuzzes
  it against `main` for 30 minutes and fails when the fuzzer finds a crash,
  panic, or hang. The workflow can also be triggered manually with a
  configurable run duration (default 1 minute), and it files a GitHub issue
  automatically when it finds a crash, panic, or hang, with the crashing
  input attached as a run artifact and repeated failures deduplicated onto
  one issue.

### Changed

- Upgraded `actions/upload-artifact` to v7 and `actions/download-artifact` to
  v8 in the release workflow, matching the current artifact backend.
- Added Cargo build caching with `Swatinem/rust-cache` to the CI, release,
  and nightly fuzz workflows. The fuzz job now pins a specific nightly date
  so its cache key remains stable across days.
- SHA-pinned every GitHub Action in the CI, release, and nightly fuzz
  workflows (`checkout`, `rust-cache`, `upload/download-artifact`, and
  `github-script`) for supply-chain hardening; Dependabot keeps them
  current.

### Removed

- Removed the bounded fuzz-smoke CI job from push and pull-request runs;
  nightly coverage-guided fuzzing now owns deep fuzzing while the small
  dependency-free fuzz batch still runs in the ordinary test suite.

## [1.4.0] - 2026-08-25

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
