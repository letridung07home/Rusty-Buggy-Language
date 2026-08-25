# Rusty Buggy Language

A programming language written in Rust, designed to help AI coding agents work
more effectively.

## v0.2.0 scope

The first runnable language capability evaluates one integer expression passed
as the executable's only command-line argument. This release intentionally has
a narrow scope: it does not include unary negation, variables, statements,
files, stdin, floating-point values, or arbitrary-precision integers.

## Getting started

With Rust installed, run:

```bash
cargo run -- "1 + 2 * (3 + 4)"
# 15
```

The expression may contain ASCII decimal integer literals, whitespace, binary
`+`, `-`, `*`, and `/`, and parentheses. Operators follow conventional
precedence and are left-associative within each precedence level.

Arithmetic uses signed 64-bit integers. Division truncates toward zero, and
literal-range violations, arithmetic overflow, division by zero, malformed
input, and any unused input are errors. Errors are printed as `error: <message>`
to stderr and the process exits unsuccessfully without printing a result.

The project uses GitHub Actions for formatting, compilation, tests, and Clippy.
Releases are created from `v*` tags, with their descriptions populated from
the matching entry in [CHANGELOG.md](CHANGELOG.md).

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
