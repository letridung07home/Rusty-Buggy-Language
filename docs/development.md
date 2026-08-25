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
- `docs/development.md` (this guide) records implementation and maintenance
  decisions.
- `CONTRIBUTING.md` covers contribution workflow and releases.
- `CHANGELOG.md` records versioned history, not the current specification.

## Implementation overview

The executable follows a small, direct pipeline:

```text
command-line argument
        |
        v
lexer -> tokens -> parser -> program AST -> evaluator -> integer result
```

- `src/main.rs` owns the command-line interface: argument-count validation,
  help and version flags, standard-output/error behavior, and exit status.
- `src/expression.rs` contains the language implementation. Its lexer turns
  text into tokens; its recursive-descent parser builds declarations and
  expressions according to precedence; its evaluator resolves immutable
  variables and performs checked `i64` arithmetic.
- Unit tests next to the implementation cover syntax and evaluation behavior.
  `tests/cli.rs` covers the compiled executable's output and exit status.
- `.github/workflows/ci.yml` checks formatting, compilation, tests, Clippy,
  and Rust documentation on stable Rust, then checks compilation and tests on
  the minimum supported Rust version. `.github/workflows/release.yml` creates
  releases from validated version tags and extracts their descriptions from
  `CHANGELOG.md`.

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

Keep the language intentionally narrow unless a feature has a clear benefit
for AI coding agents. Files, standard input, comments, floating-point values,
unary plus, and arbitrary-precision integers are not currently supported;
adding any of them requires a deliberate specification update rather than a
documentation-only change.

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
