# Tutorial

This tutorial walks through writing and running Rusty Buggy Language programs.
It assumes you have [built and installed the evaluator](../CONTRIBUTING.md) or
are running it through `cargo run -- <program>`. The language is a small,
agent-friendly expression language: a program is zero or more immutable `let`
declarations followed by exactly one final integer expression.

## Your first program

Pass a program inline:

```bash
cargo run -- "1 + 2 * (3 + 4)"
# 15
```

Multiplication binds tighter than addition, so this is `1 + (2 * (3 + 4))`.

## Arithmetic operators

The language supports `+`, `-`, `*`, `/`, and `%` on signed 64-bit integers.
Division truncates toward zero, and `%` is the remainder of truncated
division, so its sign follows the dividend:

```bash
cargo run -- "7 / 3"
# 2

cargo run -- "7 % 3"
# 1

cargo run -- "-7 % 3"
# -1
```

All arithmetic is checked: overflow, division by zero, and
`-9223372036854775808 % -1` are reported as errors rather than producing
garbage.

## Parentheses and negation

Use parentheses to override precedence, and a leading `-` to negate:

```bash
cargo run -- "(1 + 2) * 3"
# 9

cargo run -- "--1"
# 1
```

## Immutable variables

Declare immutable integers with `let`. Each declaration is available to later
declarations and to the final expression, but never to earlier ones, and it
cannot be redefined:

```bash
cargo run -- "let rate = 20; let quantity = 5; rate * quantity"
# 100
```

## Comparisons

Each comparison evaluates to `1` (true) or `0` (false), so results can be
stored and used in arithmetic:

```bash
cargo run -- "let ready = 3 >= 2; ready * 10"
# 10
```

Comparisons cannot be chained; use parentheses for separate checks, as in
`(1 < 2) == (2 < 3)`.

## Comments

`//` starts a line comment and `/* ... */` a block comment. Both are stripped
before evaluation:

```bash
cargo run -- "1 /* group */ + 2 * 3 // note"
# 7
```

## Source files and standard input

Write a program to a `.rbl` file and run it directly, matching the training
examples in this repository's `examples/` directory:

```bash
cargo run -- --file examples/fahrenheit.rbl
# 212
```

Or pipe standard input:

```bash
echo "let a = 6; let b = 7; a * b" | cargo run -- --stdin
# 42
```

## The interactive REPL

For quick experiments, start the read-evaluate-print loop. It reads one
program per line and prints each result:

```bash
cargo run -- --repl
> 1 + 2
3
> let x = 9; x / 2
4
> (press Ctrl-D to exit)
```

## Example programs

The `examples/` directory contains runnable programs for each feature:

- `hello.rbl` — the simplest expression.
- `arithmetic.rbl` — every arithmetic operator.
- `comparisons.rbl` — comparisons as `1`/`0`.
- `fahrenheit.rbl` — an inline conversion.
- `session.rbl` — a multi-step immutable-binding program.

Run any of them with:

```bash
cargo run -- --file examples/session.rbl
```

For the complete grammar, precedence, resource limits, and the full list of
error conditions, see the [language reference](language.md).