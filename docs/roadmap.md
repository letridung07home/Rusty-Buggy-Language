# Roadmap

This roadmap records the goals for each release milestone. It is not the
language specification: supported behavior is defined by the
[language reference](language.md), and completed changes are recorded in the
[changelog](../CHANGELOG.md).

## v1.0 — Stable core language

**Goal:** Deliver Rusty Buggy Language as a stable, predictable,
agent-friendly integer expression language with a supported Rust library API
and command-line interface.

### Goals

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

### Non-goals

Control flow, functions, mutation, comments, floating-point values, additional
numeric types, and a standard library are not v1 goals unless a concrete agent
workflow proves they are essential to the stable core.

## v2.0 — Next milestone

Goals for v2.0 are not yet defined. Planning begins once the v1 goals above
are settled.
