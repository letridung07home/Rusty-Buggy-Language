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

**Status:** All v1.x development goals are delivered. The series closes with
v1.6.0; further language evolution is planned for v2.0.

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
- [x] Attach prebuilt Linux, macOS, and Windows binaries to each GitHub
  release so agents can install without a Rust toolchain.
- [x] Add a tutorial (`docs/tutorial.md`) and an `examples/` directory of
  runnable programs.

Publishing the crate to crates.io was originally a development goal but was
deliberately dropped before the series closed; distribution stays with the
prebuilt GitHub-release binaries.

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

## v2.x — Typed expression language series

**Goal:** Turn Rusty Buggy Language from an integer-only expression language
into a small typed language with booleans, strings, control flow, functions,
and an agent-facing toolkit, while keeping it stable, predictable, and
agent-friendly. The v2 series deliberately breaks the v1.0 contract; the
headline change is a public `Value` type in place of the bare `i64` result.

**Status:** The v2.0.0 foundation (richer values) is delivered: the public
`Value` type, booleans, strings, `if`/`else`, and the static type checker are
shipped and documented. Functions, the agent-facing toolkit, and the remaining
stability items are still open. The task list below is the v2 backlog; each
release picks the tasks it will ship, and tasks are not bound to a specific
version.

### Series commitments

- The v1 series is closed: language semantics, the public library API, CLI
  behavior, and documented error messages may change between v1.x and the
  first v2 release.
- Every v2 change is specified in the language reference, covered by tests,
  and recorded in the changelog, with the README, CLI help, and tutorial kept
  in sync.
- Once the first v2 release ships, the v2.x series stays backward compatible
  with that contract, enforced by the CI semver job against the first v2 tag
  as baseline (bumped to each new 2.x tag when cutting a release).
- Rust 1.70 remains the minimum supported version unless a v2 feature
  genuinely requires newer `std`; raising it is a deliberate, documented
  decision made at release time, not an incidental side effect.

### Development goals

Concrete deliverables for the v2 series, grouped by theme and ordered by
priority. Each one updates the language reference and changelog on
completion. Developers choose which tasks ship in each release.

**Richer values (breaking foundation)**

- [x] Introduce a public `Value` enum (`Int(i64)`, `Bool(bool)`,
  `String(String)`) and change `evaluate` and `evaluate_with_limits` to
  return `Result<Value, Error>`. `Error`, `SourcePosition`, and `Limits`
  stay unchanged, and the CLI prints each value via its `Display` impl
  (integers, `true`/`false`, and strings without surrounding quotes).
- [x] Add `true`/`false` literals and make comparisons produce a real
  boolean instead of integer `1`/`0`; add prefix `!` and short-circuiting
  `&&`/`||`. This changes existing programs such as
  `let ready = 3 >= 2; ready * 10`, which becomes a type error.
- [x] Add `"..."` string literals with `\n`, `\t`, `\\`, and `\"` escapes;
  `+` concatenates strings and `==`/`!=` compares them.
- [x] Add `if`/`else` expressions with expression blocks
  (`{ let ...; expression }`) and lexical scoping; `else` is required and
  both branches must have the same type.
- [x] Add a lightweight static type checker so ill-typed programs fail with
  a positioned type error before evaluation instead of at runtime.
- [x] Update the property-based reference model for boolean comparison
  results plus boolean and string coverage; keep the fuzz targets on the
  `evaluate` entry point.

**Functions**

- [ ] Add function declarations `fn name(param, ...) = expression;` that
  later declarations and the final expression can call; bodies may be
  blocks. Parameters are immutable and scoped to the body.
- [ ] Support recursion, with monomorphic type inference computed by
  fixed-point iteration over the small type lattice so parameter and result
  types are inferred from bodies and call sites.
- [ ] Report positioned errors for calling an undefined function, passing
  the wrong number of arguments, and call-site type mismatches.
- [ ] Extend the property-based reference model to generated function
  programs.

**Agent-facing toolkit**

- [ ] Add the first standard-library functions: `len` (string length in
  characters), `int_to_string`, `string_to_int` (rejecting non-integer text
  with a positioned error), `bool_to_int` (`1`/`0`), and `int_to_bool`
  (`0` is false, any other value is true), plus lexicographic ordering
  comparisons (`<`, `<=`, `>`, `>=`) on strings.
- [ ] Add a `--json` CLI mode that prints the result, or the error message
  and position, as machine-readable JSON so agents can consume output
  without parsing prose.
- [ ] Improve error rendering: `--positions` gains a source snippet showing
  the offending line with a caret under the error column.

**Stability and distribution**

- [x] Switch the CI semver job from a v1 baseline to `release-type: major`
  while the breaking v2 window is open (the first half of this item; switching
  the baseline to the first v2 tag happens when v2.0.0 ships), so later 2.x
  releases stay backward compatible.
- [x] Add runnable examples and tutorial sections for booleans, strings, and
  control flow as those features land (the functions examples and tutorial
  sections land with the functions group).

### Deferred beyond v2

- **Imperative loops.** Iteration comes from recursion, so `while` loops are
  deferred beyond v2 rather than forcing mutation into the language.
- **Collections.** Lists, maps, and string indexing are candidates for a
  later 2.x release but are not scheduled.
- **A general-purpose standard library** (files, processes, networking) is
  out of scope; the v2 standard library stays a small agent-relevant set.

### Non-goals for the v2 series

Mutation and reassignment, floating-point values, additional numeric types,
and objects or records are not planned for v2.
