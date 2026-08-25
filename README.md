# Rusty Buggy Language

A programming language written in Rust, designed to help AI coding agents work
more effectively.

## v0.5.0 scope

The command-line evaluator supports immutable integer variables. A program
contains zero or more sequential `let` declarations followed by exactly one
final expression. Declaration initializers may reference earlier variables,
but variables cannot be reassigned, redeclared, or referenced before their
declaration. The `let` keyword is reserved.

The evaluator reports `rusty-buggy-language 0.5.0` when passed `-V` or
`--version` as its only argument. These flags cannot be combined with a
program or any other argument.

## v0.3.0 scope

The language evaluates one integer expression passed as the executable's only
command-line argument. This release adds prefix unary negation while retaining
the intentionally narrow scope: files, stdin,
floating-point values, unary plus, and arbitrary-precision integers remain
deferred.

## Getting started

With Rust installed, run:

```bash
cargo run -- "1 + 2 * (3 + 4)"
# 15

cargo run -- "-(1 + 2) * -3"
# 9

cargo run -- "let rate = 20; let quantity = 5; rate * quantity"
# 100

cargo run -- --help
# Usage: rusty-buggy-language "<program>"
#        rusty-buggy-language -h | --help
#        rusty-buggy-language -V | --version
#
# Evaluates an i64 integer program with immutable let bindings, +, -, *, /, parentheses, and prefix -.

cargo run -- --version
# rusty-buggy-language 0.5.0
```

The program may contain ASCII decimal integer literals, whitespace, immutable
declarations, identifiers matching `[A-Za-z_][A-Za-z0-9_]*`, binary `+`, `-`,
`*`, and `/`, prefix unary `-`, and parentheses. A declaration has the form
`let <name> = <expression>;`. Unary negation binds tighter than multiplication
and division and associates right-to-left, so `-1`, `1 * -2`, `-(1 + 2)`, and
`--1` are valid. Unary `+` is invalid. Binary operators are left-associative
within each precedence level.

Arithmetic uses signed 64-bit integers. Division truncates toward zero, and
literal-range violations, undefined or duplicate variables, arithmetic
overflow, division by zero, malformed input, and any unused input are errors.
The literal `-9223372036854775808` is accepted as `i64::MIN`; its unnegated
magnitude is out of range. Errors are printed as `error: <message>` to stderr
and the process exits unsuccessfully without printing a result.

The project uses GitHub Actions for formatting, compilation, tests, and Clippy.
Releases are created from `v*` tags, with their descriptions populated from
the matching entry in [CHANGELOG.md](CHANGELOG.md).

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
