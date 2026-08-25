# Roadmap

This roadmap records the goals for each release milestone. It is not the
language specification: supported behavior is defined by the
[language reference](language.md), and completed changes are recorded in the
[changelog](../CHANGELOG.md).

## v1.x — Stable core language series

**Goal:** Keep Rusty Buggy Language a stable, predictable, agent-friendly
integer expression language with a supported Rust library API and CLI for the
lifetime of the 1.x series. Every 1.x change must remain backward compatible
with the v1.0.0 contract; anything that would break it is deferred to v2.

### Series commitments

- No breaking changes to language semantics, the public
  `evaluate(&str) -> Result<i64, Error>` API, CLI behavior, or documented
  error messages within the 1.x series.
- Every new feature or fix is specified in the language reference, covered by
  tests, and recorded in the changelog, with the README and CLI help kept in
  sync.
- Every 1.x release passes the GitHub Actions validation suite on stable Rust
  and Rust 1.70 and ships from an annotated tag with changelog-imported
  release notes.

### Development goals

Concrete deliverables for the 1.x series, ordered by priority. Each one
updates the language reference and changelog on completion.

**Hardening the evaluator (essential)**

- [x] Add a parser nesting-depth limit that reports a clear
  "program too deeply nested" error instead of overflowing the stack on
  adversarial input.
- [x] Add a configurable input-size limit so a single program cannot exhaust
  memory or evaluation time.
- [x] Add source positions (line and column) to error output so failures can
  be located in the input.
- [x] Add property-based tests that check evaluation against a reference
  model, and a fuzzing harness run in CI for bounded time.

**Agent-friendly language additions (backward compatible)**

- [x] Add `//` line comments and `/* */` block comments, stripped by the
  lexer so they never affect evaluation.
- [x] Add the modulo operator `%` with the same checked semantics as `/`
  (truncating toward zero, with division-by-zero and overflow errors).

**Developer experience and distribution**

- [x] Add an interactive REPL mode (`rusty-buggy-language --repl`) that reads
  programs line by line and prints each result, reusing the exact CLI
  evaluator.
- [ ] Publish the crate to crates.io and keep package versions in sync with
  every 1.x release. Requires an automated publish step executed in GitHub
  Actions.
- [x] Attach prebuilt Linux, macOS, and Windows binaries to each GitHub
  release so agents can install without a Rust toolchain.
- [x] Add a tutorial (`docs/tutorial.md`) and an `examples/` directory of
  runnable programs.

### v1.0.0 milestone — delivered

- [x] Freeze and document the core language contract: immutable `let`
  declarations; signed, checked `i64` arithmetic; comparison expressions
  returning `1` or `0`; and defined operator precedence, declaration
  visibility, and error behavior.
- [x] Maintain the public library API:
  `evaluate(&str) -> Result<i64, Error>`.
- [x] Ensure identical evaluation semantics for inline programs, UTF-8 files,
  and UTF-8 standard input.
- [x] Provide clear, stable errors for invalid input, undefined variables,
  duplicate declarations, arithmetic overflow, and division by zero.
- [x] Keep the README, language reference, CLI help, API documentation, and
  changelog consistent with the implementation.
- [x] Pass the full GitHub Actions validation suite on stable Rust and
  Rust 1.70.
- [x] Produce a reproducible `v1.0.0` GitHub release from an annotated tag,
  with release notes imported from `CHANGELOG.md`.

### v1.1.0 milestone — delivered

- [x] Add source positions (line and column) to evaluation and syntax errors,
  exposed through `Error::position()` and reported by the CLI with the
  opt-in `--positions` flag, without changing existing error messages.
- [x] Add a parser nesting-depth limit that rejects programs nested deeper
  than 256 levels with a clear "program too deeply nested" error instead of
  overflowing the stack on adversarial input.
- [x] Add a configurable input-size limit via `evaluate_with_limits` and the
  CLI `--input-limit <bytes>` flag so a single program cannot exhaust memory
  or evaluation time.
- [x] Keep the evaluator-hardening features backward compatible with the
  v1.0.0 contract and pass the full GitHub Actions validation suite on
  stable Rust and Rust 1.70.
- [x] Produce a `v1.1.0` GitHub release from an annotated tag, with release
  notes imported from `CHANGELOG.md`.

### Non-goals for the 1.x series

Mutation, control flow, functions, floating-point values, additional numeric
types, strings, bitwise operators, and a standard library are not 1.x goals;
they are candidates for v2. Any feature that would require breaking the v1
contract is deferred to v2.

## v2.0 — Next milestone

Goals for v2.0 are not yet defined. Planning begins once the v1.x goals above
are settled.
