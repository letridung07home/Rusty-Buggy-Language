# Language reference

Rusty Buggy Language is a small expression language evaluated by the
`rusty-buggy-language` command-line program. A program consists of zero or more
immutable variable declarations followed by exactly one final expression.
Values are signed 64-bit integers, booleans, and UTF-8 strings.

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
| `--positions` | Also report the line and column of an evaluation, type, or syntax error. When set, the CLI prints `error: <message>` to standard error followed by ` at line L, column C`. Without it, error output is exactly the plain `error: <message>` line. |
| `--input-limit <bytes>` | Reject a source program longer than `<bytes>` bytes before it is parsed or evaluated. The value must be a non-negative integer. |

## Resource limits

The evaluator applies two built-in limits so that adversarial or oversized
input reports a clear error instead of exhausting memory or the call stack.

- **Nesting depth.** Parenthesized expressions, chains of prefix `-`/`!`, and
  nested `if`/`else` blocks may not nest deeper than 128 levels. Exceeding the
  limit reports `program too deeply nested`. Ordinary programs (even with heavy
  parentheses) are well below this.
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
block          ::= "{" declaration* expression "}"
expression     ::= logical_or
logical_or     ::= logical_and ("||" logical_and)*
logical_and    ::= comparison ("&&" comparison)*
comparison     ::= additive (comparison_operator additive)?
comparison_operator ::= "<" | "<=" | ">" | ">=" | "==" | "!="
additive       ::= multiplicative (("+" | "-") multiplicative)*
multiplicative ::= unary (("*" | "/" | "%") unary)*
unary          ::= ("-" | "!") unary | primary
primary        ::= integer | string | "true" | "false" | identifier
                 | "(" expression ")" | if_expression
if_expression  ::= "if" expression block "else" block
integer        ::= digit+
string         ::= '"' (escape | character)* '"'
escape         ::= "\\n" | "\\t" | "\\\\" | "\\\""
identifier     ::= (ascii_letter | "_")
                   (ascii_letter | ascii_digit | "_")*
```

Integer literals contain ASCII decimal digits only; digit separators and other
numeric bases are not supported. String literals are enclosed in double
quotes; the escapes `\n`, `\t`, `\\`, and `\"` are decoded by the lexer, any
other escape sequence is an error, and a literal containing a raw newline or
running past the end of the input is unterminated. Identifiers use ASCII
letters, digits, and underscores, but cannot start with a digit. The exact
identifiers `let`, `true`, `false`, `if`, and `else` are reserved keywords and
cannot be used as variable names.

## Values

A program evaluates to exactly one value of one of three types.

### Integers

All integer values are signed 64-bit integers in the range
`-9223372036854775808` to `9223372036854775807`. Arithmetic is checked.
Addition, subtraction, multiplication, and unary negation report an error when
their result is outside that range. Division truncates toward zero; division
by zero and dividing `-9223372036854775808` by `-1` are errors. The `%`
operator returns the remainder of truncated division, so its sign follows the
dividend; it has the same checked semantics as `/`, and modulo by zero or
`-9223372036854775808 % -1` are errors.

The `-` in a negative literal is parsed as prefix unary negation. The special
literal `-9223372036854775808` is accepted, while its unnegated magnitude is
not a valid value. Other out-of-range literal magnitudes are rejected. Unary
`+` is not supported.

### Booleans

The literals `true` and `false` produce booleans. Each comparison evaluates to
the boolean `true` when it holds and `false` otherwise:

```text
let ready = 3 >= 2;
if ready { 10 } else { 0 }
```

This program evaluates to `10`. Because comparisons return real booleans, a
comparison result cannot be used directly in arithmetic: `let ready = 3 >= 2;
ready * 10` is a type error. Use an `if` expression to branch on it instead.

The prefix `!` negates a boolean, and `&&` and `||` combine booleans with
short-circuit evaluation: the right operand of `&&` is not evaluated when the
left operand is `false`, and the right operand of `||` is not evaluated when
the left operand is `true`.

The standalone `!` and `=` tokens are not comparison operators. Use `!=` for
inequality, `==` for equality, and a single `=` only in a `let` declaration.

### Strings

String literals produce UTF-8 strings. The `+` operator concatenates two
strings, and `==`/`!=` compare strings:

```text
let greeting = "hello" + " " + "world";  // "hello world"
let same = greeting == "hello world";    // true
```

String ordering comparisons (`<`, `<=`, `>`, `>=`) are not yet supported;
they are planned for a later v2 release.

## if/else expressions

An `if` expression chooses between two blocks based on a boolean condition:

```text
let temperature = 32;
let verdict = if temperature > 30 { "hot" } else { "cold" };
```

The `else` branch is required, the condition must be a boolean, and both
branches must produce the same type. Blocks are `{ declaration* expression }`:
they may declare local variables and must end in exactly one expression. `if`
is an ordinary expression, so it can appear anywhere an expression can, such
as a declaration initializer or inside another `if`.

Declarations inside a block are scoped to that block and are not visible
outside it. A block may shadow a name declared in an enclosing scope; within a
single scope (the program top level or one block), a name cannot be declared
twice. Names resolve to the innermost enclosing declaration.

## Declarations and evaluation

Declarations are evaluated from left to right and bind an immutable value:

```text
let first = 2;
let second = first + 3;
second * 4
```

An initializer can refer only to declarations that appear earlier in the same
scope, or in an enclosing scope. Variables cannot be reassigned. The final
expression is evaluated after all declarations; a program containing only
declarations is incomplete.

## Type checking

Before evaluation, a static type checker walks the program and rejects
ill-typed programs with a positioned error. The type rules are:

- `+` accepts two integers (adding) or two strings (concatenating).
- `-`, `*`, `/`, and `%` accept two integers.
- `<`, `<=`, `>`, and `>=` accept two integers and produce a boolean.
- `==` and `!=` accept two values of the same type and produce a boolean.
- `&&` and `||` accept two booleans and produce a boolean.
- prefix `-` accepts an integer; prefix `!` accepts a boolean.
- An `if` condition must be a boolean, and both `if` branches must have the
  same type.
- References to undeclared variables are errors.

Type-error messages name the offending operator, the expected operand types,
and the types found, for example
`type mismatch in '+': expected two integers or two strings, found integer and
boolean`. The type names used in messages are `integer`, `boolean`, and
`string`.

## Operator precedence

Operators on the same row associate as shown, and parentheses can override the
default order:

| Precedence | Operators | Associativity |
| ---: | --- | --- |
| Highest | prefix `-`, prefix `!` | right-to-left |
| 4 | `*`, `/`, `%` | left-to-right |
| 3 | `+`, `-` | left-to-right |
| 2 | `<`, `<=`, `>`, `>=`, `==`, `!=` | at most one per expression level |
| 1 | `&&` | left-to-right |
| Lowest | `||` | left-to-right |

For example, `-2 * 3 + 4` evaluates as `((-2) * 3) + 4`, while `10 - 3 - 2`
evaluates as `(10 - 3) - 2`. Arithmetic binds more tightly than comparisons,
so `1 + 2 < 4 * 2` evaluates as `(1 + 2) < (4 * 2)`, and `&&` binds more
tightly than `||`, so `true || false && false` evaluates as
`true || (false && false)`.

An expression level may contain at most one comparison operator. Chained or
mixed comparisons such as `1 < 2 < 3` and `1 < 2 == 1` are rejected. Use
`&&`/`||` or parentheses to combine comparisons, as in
`1 < 2 && 2 < 3` or `(1 < 2) == (2 < 3)`.

## Library API

Programs can also be evaluated through the Rust library:

```rust
let result = rusty_buggy_language::evaluate("1 + 2")?;
```

`evaluate` (and `evaluate_with_limits`) return `Result<Value, Error>`, where
`Value` is a typed result: `Int(i64)`, `Bool(bool)`, or `String(String)`.
Integer programs produce `Value::Int`, booleans produce `Value::Bool`, and
strings produce `Value::String`, matching the language rules above. `Error`
and `SourcePosition` are unchanged from v1.

## Errors

On an invalid invocation or program, the CLI prints `error: <message>` to
standard error, prints no result, and exits unsuccessfully. Errors include:

- missing or extra command-line arguments;
- a missing file path, conflicting source modes, or an unreadable source file;
- inline, file, or standard-input source that is not valid UTF-8;
- an empty expression, malformed declarations, or a missing final expression;
- unexpected characters, unsupported unary `+`, unmatched parentheses, chained
  comparisons, or trailing input;
- an unterminated block comment;
- an unterminated string literal or an invalid escape sequence in a string
  literal;
- integer literals outside the supported range;
- undefined or forward-referenced variables and duplicate declarations;
- type errors such as mixing operands of different types, a non-boolean `if`
  condition, or `if` branches of different types;
- arithmetic overflow, including negating the minimum integer, dividing it
  by `-1`, or computing `-9223372036854775808 % -1`;
- division by zero and modulo by zero;
- a source program longer than the configured input limit; and
- expressions or prefix `-`/`!` chains nested more deeply than the nesting
  limit.

When `--positions` is supplied, the CLI appends ` at line L, column C` to the
error so the failure can be located in the source.

For example:

```text
error: division by zero
error: undefined variable: 'missing'
error: integer addition overflow
error: type mismatch in '+': expected two integers or two strings, found integer and boolean
error: if branches must have the same type, found integer and string
```
