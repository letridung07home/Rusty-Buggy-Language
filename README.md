# Rusty Buggy Language

A programming language written in Rust, designed to help AI coding agents work
more effectively.

## v0.3.0 scope

The language evaluates one integer expression passed as the executable's only
command-line argument. This release adds prefix unary negation while retaining
the intentionally narrow scope: variables, statements, files, stdin,
floating-point values, unary plus, and arbitrary-precision integers remain
deferred.

## Getting started

With Rust installed, run:

```bash
cargo run -- "1 + 2 * (3 + 4)"
# 15

cargo run -- "-(1 + 2) * -3"
# 9
```

The expression may contain ASCII decimal integer literals, whitespace, binary
`+`, `-`, `*`, and `/`, prefix unary `-`, and parentheses. Unary negation binds
tighter than multiplication and division and associates right-to-left, so
`-1`, `1 * -2`, `-(1 + 2)`, and `--1` are valid. Unary `+` is invalid. Binary
operators are left-associative within each precedence level.

Arithmetic uses signed 64-bit integers. Division truncates toward zero, and
literal-range violations, arithmetic overflow, division by zero, malformed
input, and any unused input are errors. The literal `-9223372036854775808` is
accepted as `i64::MIN`; its unnegated magnitude is out of range. Errors are
printed as `error: <message>` to stderr and the process exits unsuccessfully
without printing a result.

The project uses GitHub Actions for formatting, compilation, tests, and Clippy.
Releases are created from `v*` tags, with their descriptions populated from
the matching entry in [CHANGELOG.md](CHANGELOG.md).

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
