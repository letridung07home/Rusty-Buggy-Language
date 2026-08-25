# Development guide

This guide describes how the Rusty Buggy Language implementation is organized
and how to evolve it without losing its deliberately small, well-specified
scope. For setup, contribution expectations, and the release checklist, see
[CONTRIBUTING.md](../CONTRIBUTING.md). For the language's user-facing
contract, see the [language reference](language.md).

## Documentation responsibilities

Keep each document focused on its audience:

- `README.md` introduces the project and shows how to run it.
- `docs/language.md` is the authoritative description of currently supported
  syntax and behavior.
- `docs/roadmap.md` records intended release goals and their scope.
- `docs/development.md` (this guide) records implementation and maintenance
  decisions.
- `CONTRIBUTING.md` covers contribution workflow and releases.
- `CHANGELOG.md` records versioned history, not the current specification.
- `docs/tutorial.md` introduces the language with runnable commands, and the
  `examples/` directory holds runnable programs; both complement the README.

## Implementation overview

The library and executable share a small, direct pipeline:

```text
library evaluate(program)
        |
        v
lexer -> tokens -> parser -> program AST -> evaluator -> integer result
        ^
        |
private CLI adapter <- process startup
        ^
        |
source selection and complete UTF-8 input read
```

- `src/lib.rs` owns the public library boundary and orchestration. Its primary
  public function is `evaluate(program: &str) -> Result<i64, Error>`, with
  `evaluate_with_limits(program, &Limits)` exposing the configurable input-size
  bound. `evaluate` uses `Limits::default()`. Public types are `Error`, the
  opaque error whose `Display` preserves the existing user-facing message, and
  `SourcePosition`, exposed through `Error::position()`.
- `src/ast.rs`, `src/lexer.rs`, `src/parser.rs`, `src/evaluator.rs`, and
  `src/error.rs` are flat, private implementation modules. The lexer turns
  text into position-tagged tokens; the recursive-descent parser builds
  declarations and expressions according to precedence and stamps each AST
  node with the source position of its first token; and the evaluator resolves
  immutable variables and performs checked `i64` arithmetic, attaching the
  relevant node's position to evaluation errors.
- Resource limits live on the parser and the library entry point. The parser
  bounds recursive nesting (parentheses and prefix `-` chains) with a `depth`
  counter, reporting `program too deeply nested`; the library entry points
  enforce the byte-length input limit from `Limits` before lexing. Positions
  are always tracked internally, but CLI output only shows them when the
  `--positions` flag is passed, keeping the default error text stable.
- `src/main.rs` only supplies process arguments and delegates the exit code.
- Unit tests next to each pipeline stage cover lexical, syntax, and evaluation
  behavior. Library-facade tests cover the public API and error display.
  `src/cli.rs` tests cover source selection, the REPL, and injected input
  reads and output, while `tests/cli.rs` remains the executable-contract
  suite for inline, file, standard-input, and REPL behavior, output, and exit
  status.
- `tests/property_reference.rs` holds a self-contained property-based test
  that renders generated programs back to source and requires the full
  pipeline to agree with an independent reference evaluator over checked
  `i64` arithmetic and ordered immutable `let` bindings.
- `tests/fuzz_smoke.rs` is a dependency-free fuzzing harness over the public
  `evaluate` entry point (random token soup, pseudo-programs, arbitrary
  UTF-8, and edge cases), with a fast batch that runs inside the ordinary
  test suite.
- `fuzz/` holds a `cargo-fuzz` coverage-guided fuzz target over the public
  `evaluate` entry point, with a small committed seed corpus of arithmetic
  edge cases. It requires the nightly toolchain (libFuzzer sanitizer
  coverage), so it runs in a dedicated nightly workflow rather than the
  stable/MSRV suites.
- `.github/workflows/ci.yml` checks formatting, compilation, tests, Clippy,
  and Rust documentation on stable Rust, then checks compilation and tests
  on the minimum supported Rust version.
  `.github/workflows/nightly-fuzz.yml` runs the coverage-guided `cargo-fuzz`
  target against `main` for 30 minutes every night, failing when the fuzzer
  finds a crash, panic, overflow, or hang; such failures file a GitHub issue
  automatically with the crashing input attached. It can also be dispatched
  manually with a configurable duration in minutes (default 1) for a quick
  check.
  `.github/workflows/release.yml` creates releases from validated version tags,
  extracts their descriptions from `CHANGELOG.md`, and attaches prebuilt
  Linux, macOS, and Windows `x86_64` binaries built from the tag. All three
  workflows cache Cargo build artifacts with `Swatinem/rust-cache`; the fuzz
  job pins a specific nightly date so its cache key stays stable across days.

## Evolving the language

Treat `docs/language.md` as part of the language contract. Any behavior change
should update the implementation, its focused test coverage, the reference,
and the `Unreleased` section of `CHANGELOG.md` together.

When adding a syntactic feature, consider each stage explicitly:

1. Define its grammar and interaction with existing precedence and
   associativity rules.
2. Extend the lexer token set only when the spelling cannot reuse an existing
   token.
3. Extend the parser and AST so invalid forms produce clear errors.
4. Implement evaluation with checked arithmetic and the current declaration
   visibility rules in mind.
5. Add unit and CLI coverage for successful use, invalid syntax, and relevant
   error paths.

Comments are handled entirely by the lexer: `//` and `/* ... */` sequences
are recognized and stripped while the token stream is produced, so they never
reach the parser, AST, or evaluator. Their behavior belongs in the language
reference rather than the grammar.

Keep the language intentionally narrow unless a feature has a clear benefit
for AI coding agents. Floating-point values, unary plus, and arbitrary-
precision integers are not currently supported; adding any of them requires a
deliberate specification update rather than a documentation-only change. File and standard-input handling belongs to the CLI adapter and must
not change the public `evaluate(&str) -> Result<i64, Error>` API.

## Compatibility and verification

The package uses Rust edition 2021 and supports Rust 1.70 or later. Changes
must remain compatible with both the current stable toolchain and the minimum
supported Rust version (MSRV).

GitHub Actions is the source of truth for validation. It runs the checks listed
in [CONTRIBUTING.md](../CONTRIBUTING.md) on pushes and pull requests, including
the stable quality suite and the Rust 1.70 compatibility suite. In the
maintainer development environment, do not run Rust builds or tests locally;
push the branch and inspect the CI run instead:

```bash
gh run list --workflow CI
gh run view <run-id> --log-failed
```

## Releases

Follow the versioning and tag procedure in [CONTRIBUTING.md](../CONTRIBUTING.md).
In particular, the annotated `vX.Y.Z` tag must match the package version, and
the release workflow uses the corresponding changelog section as the GitHub
release description.
