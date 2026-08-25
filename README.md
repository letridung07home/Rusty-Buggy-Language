# Rusty Buggy Language

A programming language written in Rust, designed to help AI coding agents work
more effectively.

The command-line evaluator accepts an inline program, a UTF-8 source file, or
UTF-8 standard input. Each program contains zero or more sequential, immutable
`let` declarations followed by one final integer expression. It supports ASCII
decimal integer literals, identifiers, parentheses, prefix `-`, the comparison
operators `<`, `<=`, `>`, `>=`, `==`, and `!=`, and the arithmetic operators `+`,
`-`, `*`, and `/` with checked signed 64-bit arithmetic. Comparisons produce
integer `1` for true and `0` for false, so their results can be stored in
variables or used in later arithmetic.

For the complete grammar, evaluation rules, operator precedence, CLI behavior,
and error conditions, see the [language reference](docs/language.md). The
[roadmap](docs/roadmap.md) defines upcoming release goals, while historical
changes are recorded in the [changelog](CHANGELOG.md).

## Getting started

With Rust installed, run:

```bash
cargo run -- "1 + 2 * (3 + 4)"
# 15

cargo run -- "-(1 + 2) * -3"
# 9

cargo run -- "let rate = 20; let quantity = 5; rate * quantity"
# 100

cargo run -- "let ready = 3 >= 2; ready * 10"
# 10

cargo run -- --file program.rbl

cargo run -- --stdin < program.rbl

cargo run -- --help
cargo run -- --version
```

The source-selection modes are mutually exclusive. File and standard-input
sources are read in full as UTF-8 and passed unchanged to the same evaluator as
inline programs.

See [CONTRIBUTING.md](CONTRIBUTING.md) for repository setup, validation, and
release guidance. See the [development guide](docs/development.md) for
implementation architecture and language-maintenance notes.

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
