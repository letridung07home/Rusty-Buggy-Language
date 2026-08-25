# Rusty Buggy Language

A programming language written in Rust, designed to help AI coding agents work
more effectively.

The current command-line evaluator accepts one program argument containing zero
or more sequential, immutable `let` declarations followed by one final integer
expression. It supports ASCII decimal integer literals, identifiers,
parentheses, prefix `-`, and the binary operators `+`, `-`, `*`, and `/` with
checked signed 64-bit arithmetic.

For the complete grammar, evaluation rules, operator precedence, CLI behavior,
and error conditions, see the [language reference](docs/language.md). Historical
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

cargo run -- --help
cargo run -- --version
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for repository setup, validation, and
release guidance. See the [development guide](docs/development.md) for
implementation architecture and language-maintenance notes.

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
