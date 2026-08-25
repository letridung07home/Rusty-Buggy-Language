# Language reference

Rusty Buggy Language is currently a small expression language evaluated by the
`rusty-buggy-language` command-line program. A program consists of zero or more
immutable variable declarations followed by exactly one final expression.

## Running a program

Pass the complete program as the executable's only command-line argument, or
select a source file or standard input:

```bash
cargo run -- "let rate = 20; let quantity = 5; rate * quantity"
# 100

cargo run -- --file program.rbl
# 100

cargo run -- --stdin < program.rbl
# 100
```

The inline form is usually quoted by the shell when a program contains spaces
or operators. `-f <path>` and `--file <path>` read the complete named file as
UTF-8. `--stdin` reads the complete standard input as UTF-8. `--repl` starts an
interactive loop that reads one program per line from standard input and
prints each result, reusing the same evaluator. When the selected text avoids
`--repl`, it is passed unchanged to the evaluator, so all three source modes
have the same language semantics. The source modes are mutually exclusive;
`-` is an ordinary file path when it follows `-f` or `--file` and does not
select standard input.

Missing paths, conflicting modes, and additional arguments are errors.

The following flags are recognized only when they are the sole argument:

| Flag | Behavior |
| --- | --- |
| `-h`, `--help` | Print usage and a short feature summary. |
| `-V`, `--version` | Print `rusty-buggy-language` followed by the package version. |
| `--repl` | Start an interactive loop that reads one program per line and prints each result. |

Two optional configuration flags may appear alongside an inline program, a
`-f`/`--file` source, or `--stdin`:

| Flag | Behavior |
| --- | --- |
| `--positions` | Also report the line and column of an evaluation or syntax error. When set, the CLI prints `error: <message>` to standard error followed by ` at line L, column C`. Without it, error output is exactly the plain `error: <message>` line. |
| `--input-limit <bytes>` | Reject a source program longer than `<bytes>` bytes before it is parsed or evaluated. The value must be a non-negative integer. |

## Resource limits

The evaluator applies two built-in limits so that adversarial or oversized
input reports a clear error instead of exhausting memory or the call stack.

- **Nesting depth.** Parenthesized expressions and chains of prefix `-` may not
  nest deeper than 256 levels. Exceeding the limit reports `program too deeply
  nested`. Ordinary programs (even with heavy parentheses) are well below this.
- **Input size.** A program is rejected with `program is too large to
  evaluate` when it is longer than the configured input limit. The default is
  `1 MiB`; the CLI `--input-limit <bytes>` flag overrides it for one
  invocation, and the library exposes the same bound through
  `evaluate_with_limits`.

## Grammar

The grammar below uses `*` for zero or more repetitions and `+` for one or
more. Whitespace may appear between tokens and is ignored. `//` line comments
run from `//` to the end of the line, and `/* */` block comments run to their
first closing `*/`; both are stripped by the lexer and never affect
evaluation. Block comments do not nest. There are no alternate literal
formats.

```text
program        ::= declaration* expression
declaration    ::= "let" identifier "=" expression ";"
expression     ::= comparison
comparison     ::= additive (comparison_operator additive)?
comparison_operator ::= "<" | "<=" | ">" | ">=" | "==" | "!="
additive       ::= multiplicative (("+" | "-") multiplicative)*
multiplicative ::= unary (("*" | "/" | "%") unary)*
unary          ::= "-" unary | primary
primary        ::= integer | identifier | "(" expression ")"

integer        ::= digit+
identifier     ::= (ascii_letter | "_")
                   (ascii_letter | ascii_digit | "_")*
```

Integer literals contain ASCII decimal digits only; digit separators and other
numeric bases are not supported. Identifiers use ASCII letters, digits, and
underscores, but cannot start with a digit. The exact identifier `let` is a
reserved keyword, so it cannot be used as a variable name.

## Declarations and evaluation

Declarations are evaluated from left to right and bind an immutable integer:

```text
let first = 2;
let second = first + 3;
second * 4
```

An initializer can refer only to declarations that appear earlier in the same
program. Variables cannot be reassigned or declared more than once. The final
expression is evaluated after all declarations; a program containing only
declarations is incomplete.

All values are signed 64-bit integers in the range `-9223372036854775808` to
`9223372036854775807`. Arithmetic is checked. Addition, subtraction,
multiplication, and unary negation report an error when their result is outside
that range. Division truncates toward zero; division by zero and dividing
`-9223372036854775808` by `-1` are errors. The `%` operator returns the
remainder of truncated division, so its sign follows the dividend; it has the
same checked semantics as `/`, and modulo by zero or
`-9223372036854775808 % -1` are errors.

The `-` in a negative literal is parsed as prefix unary negation. The special
literal `-9223372036854775808` is accepted, while its unnegated magnitude is
not a valid value. Other out-of-range literal magnitudes are rejected. Unary
`+` is not supported.

Each comparison evaluates to the integer `1` when it is true and `0` when it is
false. These comparison values can be assigned by `let` declarations and used
as operands in later arithmetic:

```text
let ready = 3 >= 2;
ready * 10
```

This program evaluates to `10`.

The standalone `!` and `=` tokens are not comparison operators. Use `!=` for
inequality, `==` for equality, and a single `=` only in a `let` declaration.

## Operator precedence

Operators on the same row associate as shown, and parentheses can override the
default order:

| Precedence | Operators | Associativity |
| ---: | --- | --- |
| Highest | prefix `-` | right-to-left |
| 3 | `*`, `/`, `%` | left-to-right |
| 2 | `+`, `-` | left-to-right |
| Lowest | `<`, `<=`, `>`, `>=`, `==`, `!=` | at most one per expression level |

For example, `-2 * 3 + 4` evaluates as `((-2) * 3) + 4`, while `10 - 3 - 2`
evaluates as `(10 - 3) - 2`. Arithmetic binds more tightly than comparisons,
so `1 + 2 < 4 * 2` evaluates as `(1 + 2) < (4 * 2)`.

An expression level may contain at most one comparison operator. Chained or
mixed comparisons such as `1 < 2 < 3` and `1 < 2 == 1` are rejected. Use
parentheses to make separate comparisons explicit, as in
`(1 < 2) == (2 < 3)`.

## Errors

On an invalid invocation or program, the CLI prints `error: <message>` to
standard error, prints no result, and exits unsuccessfully. Errors include:

- missing or extra command-line arguments;
- a missing file path, conflicting source modes, or an unreadable source file;
- inline, file, or standard-input source that is not valid UTF-8;
- an empty expression, malformed declarations, or a missing final expression;
- unexpected characters, standalone `!` or `=`, unsupported unary `+`,
  unmatched parentheses, chained comparisons, or trailing input;
- an unterminated block comment;
- integer literals outside the supported range;
- undefined or forward-referenced variables and duplicate declarations;
- arithmetic overflow, including negating the minimum integer, dividing it
  by `-1`, or computing `-9223372036854775808 % -1`;
- division by zero and modulo by zero;
- a source program longer than the configured input limit; and
- expressions or prefix `-` chains nested more deeply than the nesting limit.

When `--positions` is supplied, the CLI appends ` at line L, column C` to the
error so the failure can be located in the source.

For example:

```text
error: division by zero
error: undefined variable: 'missing'
error: integer addition overflow
```
