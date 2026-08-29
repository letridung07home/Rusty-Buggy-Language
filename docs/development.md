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
lexer -> tokens -> parser -> program AST -> type checker -> evaluator -> Value result
        ^
        |
private CLI adapter <- process startup
        ^
        |
source selection and complete UTF-8 input read
```

- `src/lib.rs` owns the public library boundary and orchestration. Its primary
  public function is `evaluate(program: &str) -> Result<Value, Error>`, with
  `evaluate_with_limits(program, &Limits)` exposing the configurable input-size
  bound. `evaluate` uses `Limits::default()`. Public types are `Value` (the
  `Int`, `Bool`, and `String` results, printed via `Display`), `Error`, the
  opaque error whose `Display` preserves the existing user-facing message, and
  `SourcePosition`, exposed through `Error::position()`.
- `src/ast.rs`, `src/lexer.rs`, `src/parser.rs`, `src/typecheck.rs`,
  `src/evaluator.rs`, and `src/error.rs` are flat, private implementation
  modules. The lexer turns text into position-tagged tokens (including
  keywords, string literals with escapes, and the logical operators); the
  recursive-descent parser builds declarations, expressions, and `if`/`else`
  blocks according to precedence and stamps each AST node with the source
  position of its first token; the static type checker walks the program
  against a stack of lexical scopes and rejects ill-typed programs with a
  positioned error; and the evaluator resolves immutable variables through the
  same scope stack, performs checked `i64` arithmetic, concatenates strings,
  short-circuits `&&`/`||`, and selects `if`/`else` branches, attaching the
  relevant node's position to evaluation errors.
- Resource limits live on the parser, the evaluator, and the library entry
  point. The parser bounds recursive nesting (parentheses, prefix `-`/`!`
  chains, and nested `if`/`else` blocks) with a `depth` counter, reporting
  `program too deeply nested`; the library entry points enforce the
  byte-length input limit from `Limits` before lexing. The evaluator adds an
  evaluation-depth guard (`MAX_EVAL_DEPTH`): stack cost is call depth times
  per-call expression descent, so it bounds total expression-evaluation depth
  at 2048 levels and reports the same `program too deeply nested` message with
  the offending expression's position. Positions are always tracked
  internally, but CLI output only shows them when the `--positions` flag is
  passed, keeping the default error text stable.
- `src/main.rs` only supplies process arguments and delegates the exit code.
- Unit tests next to each pipeline stage cover lexical, syntax, and evaluation
  behavior. Library-facade tests cover the public API and error display.
  `src/cli.rs` tests cover source selection, the REPL, and injected input
  reads and output, while `tests/cli.rs` remains the executable-contract
  suite for inline, file, standard-input, and REPL behavior, output, and exit
  status.
- `tests/property_reference.rs` holds a self-contained property-based test
  that generates well-typed programs over integers, booleans, and strings,
  renders them back to source, and requires the full pipeline to agree with an
  independent reference evaluator over checked `i64` arithmetic, string
  concatenation, short-circuiting logical operators, `if`/`else` selection,
  and ordered immutable `let` bindings.
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
  and Rust documentation on stable Rust, lints the workflow files with
  actionlint, measures line coverage with `cargo-llvm-cov` (failing below
  80%) and prints the coverage summary in the job log, then checks
  compilation and tests on the minimum supported Rust version.
  Doctests run on both toolchains (`cargo test --doc`), a `release` job
  runs the full test suite in release mode (`cargo test --release`) so the
  shipped binaries are exercised without debug assertions, and a `semver`
  job checks the public library API with `cargo-semver-checks` against the
  v2.0.0 tag as the backward-compatibility baseline, so later 2.x releases
  stay compatible with the v2 contract. Runs for the same branch or pull
  request cancel the previous in-flight run, so a newer push supersedes a
  stale one.
  `.github/workflows/nightly-fuzz.yml` runs the coverage-guided `cargo-fuzz`
  target against `main` for 30 minutes every night, failing when the fuzzer
  finds a crash, panic, overflow, or hang; such failures file a GitHub issue
  automatically with the crashing input attached. It can also be dispatched
  manually with a configurable duration in minutes (default 1, clamped to
  the 1-180 minute window) for a quick check.
  `.github/workflows/release.yml` creates releases from validated version tags,
  extracts their descriptions from `CHANGELOG.md`, and attaches prebuilt
  Linux, macOS, and Windows binaries built from the tag. Each binary is
  published under a unique platform-suffixed name
  (`rusty-buggy-language-<platform>`, with the arch appended when a platform
  ships more than one, e.g. `rusty-buggy-language-linux-arm64`) so the
  artifacts never collide, and a `SHA256SUMS` file of their checksums is
  attached so users can verify downloaded builds. The tag's release-mode
  test suite must pass before the build matrix runs, so a tag is
  self-validating, and the workflow can be re-triggered manually with
  `workflow_dispatch` by supplying an existing tag name to re-create a
  release without retagging. All three
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
4. Extend the static type checker so ill-typed forms fail before evaluation
   with a positioned type error.
5. Implement evaluation with checked arithmetic and the current scope and
   value rules in mind.
6. Add unit and CLI coverage for successful use, invalid syntax, and relevant
   error paths.

Comments are handled entirely by the lexer: `//` and `/* ... */` sequences
are recognized and stripped while the token stream is produced, so they never
reach the parser, AST, or evaluator. Their behavior belongs in the language
reference rather than the grammar.

Keep the language intentionally narrow unless a feature has a clear benefit
for AI coding agents. Floating-point values, unary plus, and arbitrary-
precision integers are not currently supported; adding any of them requires a
deliberate specification update rather than a documentation-only change. File and standard-input handling belongs to the CLI adapter and must
not change the public `evaluate(&str) -> Result<Value, Error>` API.

## Compatibility and verification

The package uses Rust edition 2021 and supports Rust 1.70 or later. Changes
must remain compatible with both the current stable toolchain and the minimum
supported Rust version (MSRV).

GitHub Actions is the source of truth for validation. It runs the checks listed
in [CONTRIBUTING.md](../CONTRIBUTING.md) on pushes, pull requests, and manual
`workflow_dispatch` runs, including the stable quality suite and the Rust 1.70
compatibility suite. In the
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
