# Roadmap

This roadmap records intended release milestones. It is not the language
specification: supported behavior is defined by the
[language reference](language.md), and completed changes are recorded in the
[changelog](../CHANGELOG.md).

## v1.0 — Stable core language

**Goal:** Deliver Rusty Buggy Language as a stable, predictable,
agent-friendly integer expression language with a supported Rust library API
and command-line interface.

### Release requirements

- Freeze and document the core language contract:
  - immutable `let` declarations;
  - signed, checked `i64` arithmetic;
  - comparison expressions returning `1` or `0`; and
  - defined operator precedence, declaration visibility, and error behavior.
- Maintain the public library API:
  `evaluate(&str) -> Result<i64, Error>`.
- Ensure identical evaluation semantics for inline programs, UTF-8 files, and
  UTF-8 standard input.
- Provide clear, stable errors for invalid input, undefined variables,
  duplicate declarations, arithmetic overflow, and division by zero.
- Keep the README, language reference, CLI help, API documentation, and
  changelog consistent with the implementation.
- Pass the full GitHub Actions validation suite on stable Rust and Rust 1.70.
- Produce a reproducible `v1.0.0` GitHub release from an annotated tag, with
  release notes imported from `CHANGELOG.md`.

### Out of scope

Control flow, functions, mutation, comments, floating-point values, additional
numeric types, and a standard library are not v1 requirements unless a
concrete agent workflow proves they are essential to the stable core.
