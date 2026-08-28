# Rusty Buggy Language

A programming language written in Rust, designed to help AI coding agents work
more effectively.

The command-line evaluator accepts an inline program, a UTF-8 source file, or
UTF-8 standard input. Each program contains zero or more sequential, immutable
`let` declarations followed by one final expression. It supports ASCII
decimal integer literals, `true`/`false` literals, `"..."` string literals
(with `\n`, `\t`, `\\`, and `\"` escapes), identifiers, parentheses, prefix
`-` and `!`, the comparison operators `<`, `<=`, `>`, `>=`, `==`, and `!=`,
short-circuiting `&&` and `||`, the arithmetic operators `+`, `-`, `*`, `/`,
and `%` with checked signed 64-bit arithmetic, `if`/`else` expressions with
`{ }` blocks and lexical scoping, function declarations
(`fn name(param, ...) = { ... };`) with recursive calls and monomorphic type
inference, and `//` line comments and `/* */` block comments. Comparisons
produce real booleans, `+` concatenates strings, and a static type checker
rejects ill-typed programs before evaluation.

Programs can also be evaluated programmatically: the library's `evaluate`
entry point returns a typed `Value` result (`Int`, `Bool`, or `String`)
printed through each value's `Display` impl (integers, `true`/`false`, and
strings without surrounding quotes).

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

cargo run -- "let ready = 3 >= 2; if ready { 10 } else { 0 }"
# 10

cargo run -- "\"hello\" + \" \" + \"world\""
# hello world

cargo run -- "let temp = 32; if temp > 30 { \"hot\" } else { \"cold\" }"
# hot

cargo run -- "fn square(x) = { x * x }; square(7)"
# 49

cargo run -- "fn fact(n) = { if n <= 1 { 1 } else { n * fact(n - 1) } }; fact(5)"
# 120

cargo run -- "1 /* group */ + 2 * 3 // note"
# 7

cargo run -- --file program.rbl

cargo run -- --stdin < program.rbl

cargo run -- --repl
> 1 + 2 * (3 + 4)
15

cargo run -- --positions "8 / (3 - 3)"
# error: division by zero
#  at line 1, column 3

cargo run -- --input-limit 100 "1 + 2 * 3"

cargo run -- --help
cargo run -- --version
```

The source-selection modes are mutually exclusive. File and standard-input
sources are read in full as UTF-8 and passed unchanged to the same evaluator as
inline programs, while `--repl` reads one program per line and prints each
result. The evaluator guards against oversized input (configurable
with `--input-limit <bytes>`) and excessively nested programs, and `--positions`
adds line and column information to errors. See the docs links below for
details.

See [CONTRIBUTING.md](CONTRIBUTING.md) for repository setup, validation, and
release guidance. See the [development guide](docs/development.md) for
implementation architecture and language-maintenance notes. Work through the
[tutorial](docs/tutorial.md) or run the programs in [`examples/`](examples/)
for ready-made illustrations of each feature.

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
