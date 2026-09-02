# Tutorial

This tutorial walks through writing and running Rusty Buggy Language programs.
It assumes you have [built and installed the evaluator](../CONTRIBUTING.md) or
are running it through `cargo run -- <program>`. The language is a small,
agent-friendly expression language: a program is zero or more immutable `let`
declarations followed by exactly one final expression, which evaluates to an
integer, a boolean, or a string.

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

Declare immutable values with `let`. Each declaration is available to later
declarations and to the final expression, but never to earlier ones, and it
cannot be redefined in the same scope:

```bash
cargo run -- "let rate = 20; let quantity = 5; rate * quantity"
# 100
```

## Booleans and comparisons

Each comparison evaluates to the boolean `true` or `false`, which combine with
`!`, `&&`, and `||`:

```bash
cargo run -- "3 >= 2"
# true

cargo run -- "1 < 2 && 2 < 3"
# true

cargo run -- "!false || false"
# true
```

`&&` and `||` short-circuit: the right side is only evaluated when it can
change the result, so `false && 8 / (3 - 3) == 1` is `false` rather than a
division-by-zero error. Because comparisons produce booleans, a comparison
result cannot be multiplied: `let ready = 3 >= 2; ready * 10` is a type error.
Branch on it with `if` instead.

Comparisons cannot be chained; combine separate checks with `&&`/`||` or
parentheses, as in `(1 < 2) == (2 < 3)`.

## Strings

String literals use double quotes, with `\n`, `\t`, `\\`, and `\"` escapes.
`+` concatenates strings, and `==`/`!=` compare them:

```bash
cargo run -- "\"hello\" + \" \" + \"world\""
# hello world

cargo run -- "\"a\" == \"b\""
# false
```

Strings print without surrounding quotes.

## if/else expressions

An `if` expression picks between two blocks. The condition must be a boolean
and both branches must produce the same type:

```bash
cargo run -- "let temp = 32; if temp > 30 { \"hot\" } else { \"cold\" }"
# hot

cargo run -- "let score = 7; if score > 5 { let bonus = 3; score + bonus } else { score }"
# 10
```

Blocks may declare local variables that are scoped to the block, and may
shadow names from an enclosing scope.

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
> 3 >= 2 && 1 < 2
true
> (press Ctrl-D to exit)
```

## Functions

Beyond variables and branching, you can declare functions with parameters and a
block body. Each function's parameter and result types are inferred from its
body and the places it is called:

```bash
cargo run -- "fn square(x) = { x * x }; square(7)"
# 49

cargo run -- "fn max(a, b) = { if a > b { a } else { b } }; max(3, 7)"
# 7

cargo run -- "fn fact(n) = { if n <= 1 { 1 } else { n * fact(n - 1) } }; fact(5)"
# 120
```

A function may call itself (recursion) or other functions. Parameters are
immutable and scoped to the body, and the body is a block so it can declare
local variables that stay inside the call. Calling an undefined function, using
the wrong number of arguments, or passing a value of the wrong type is a
type error reported before evaluation.

## Standard library functions

Seven built-in functions cover common string, number, and boolean conversions:

```bash
cargo run -- "len(\"hello\")"
# 5

cargo run -- "int_to_string(-12) + \"!\""
# -12!

cargo run -- "string_to_int(\"42\") + 1"
# 43

cargo run -- "bool_to_int(3 > 2)"
# 1

cargo run -- "int_to_bool(0)"
# false

cargo run -- "bool_to_string(3 > 2)"
# true

cargo run -- "string_to_bool(\"false\")"
# false
```

`len` counts characters (Unicode scalar values), not UTF-8 bytes, so
`len("héllo")` is `5`. `string_to_int` accepts only an optional leading `-`
followed by ASCII digits — leading zeros included — and reports
`invalid integer text: '...'` for anything else, such as
`string_to_int(" 42")` or `string_to_int("+1")`. The two are inverses, so
`string_to_int(int_to_string(7))` is `7`.

`bool_to_string` prints `"true"` or `"false"`, and `string_to_bool` accepts
only those exact texts — `string_to_bool("True")` or `string_to_bool(" 1")`
reports `invalid boolean text: '...'`. The two are inverses, so
`string_to_bool(bool_to_string(3 > 2))` is `true`.

The ordering comparisons also compare strings lexicographically:

```bash
cargo run -- "\"apple\" < \"banana\""
# true
```

## Example programs

The `examples/` directory contains runnable programs for each feature:

- `hello.rbl` — the simplest expression.
- `arithmetic.rbl` — every arithmetic operator.
- `comparisons.rbl` — comparisons as real booleans with `&&`, `||`, and `!`.
- `strings.rbl` — string concatenation, escapes, and equality.
- `branching.rbl` — `if`/`else` with scoped declarations.
- `functions.rbl` — function declarations, calls, and block bodies.
- `recursion.rbl` — recursion and mutual recursion.
- `stdlib.rbl` — the built-in functions and string ordering.
- `fahrenheit.rbl` — an inline conversion.
- `session.rbl` — a multi-step immutable-binding program.

Run any of them with:

```bash
cargo run -- --file examples/session.rbl
```

For the complete grammar, precedence, type rules, resource limits, and the full
list of error conditions, see the [language reference](language.md).
