use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rusty-buggy-language"))
        .args(arguments)
        .output()
        .expect("failed to execute the compiled CLI")
}

fn run_cli_with_stdin(arguments: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rusty-buggy-language"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute the compiled CLI");

    let mut input_handle = child
        .stdin
        .take()
        .expect("failed to open the CLI standard input");
    if let Err(write_error) = input_handle.write_all(input) {
        // The CLI rejects conflicting arguments before reading standard input
        // and may have already exited, in which case a broken pipe is expected
        // rather than a test failure.
        if write_error.kind() != std::io::ErrorKind::BrokenPipe {
            panic!("failed to write the CLI standard input: {write_error}");
        }
    }
    // Dropping the handle closes the write end so the CLI observes EOF on its
    // standard input; keeping it alive through `wait_with_output` would leave
    // a child that reads stdin blocked forever waiting for input that never
    // arrives.
    drop(input_handle);

    child
        .wait_with_output()
        .expect("failed to collect the CLI output")
}

struct TemporarySource {
    path: PathBuf,
}

impl TemporarySource {
    fn new(contents: &[u8]) -> Self {
        let unique_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rusty-buggy-language-cli-{}-{unique_id}",
            std::process::id()
        ));

        fs::write(&path, contents).expect("failed to create the temporary source file");
        Self { path }
    }

    fn as_str(&self) -> &str {
        self.path
            .to_str()
            .expect("temporary source path should be valid UTF-8")
    }
}

impl Drop for TemporarySource {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[test]
fn evaluates_expression_and_prints_result_to_stdout() {
    let output = run_cli(&["1 + 2 * (3 + 4)"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"15\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_program_with_immutable_variables() {
    let output = run_cli(&["let rate = 20; let quantity = 5; rate * quantity"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"100\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_program_from_a_file_with_both_file_flags() {
    let source = TemporarySource::new(b"let rate = 20;\nlet quantity = 5;\nrate * quantity");

    for flag in ["-f", "--file"] {
        let output = run_cli(&[flag, source.as_str()]);

        assert!(output.status.success());
        assert_eq!(output.stdout, b"100\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn evaluates_multiline_program_from_standard_input() {
    let output = run_cli_with_stdin(
        &["--stdin"],
        b"let first = 2;\nlet second = first + 3;\nsecond * 4",
    );

    assert!(output.status.success());
    assert_eq!(output.stdout, b"20\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_programs_with_comments() {
    let output = run_cli(&["1 /* part */ + 2 // note"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_commented_program_from_a_file() {
    let source = TemporarySource::new(b"let rate = 20; /* per hour */\nrate * 5 // total");
    let output = run_cli(&["--file", source.as_str()]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"100\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_commented_program_from_standard_input() {
    let output = run_cli_with_stdin(&["--stdin"], b"1 /* a */ + 2 // b\n");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn reports_unterminated_block_comments() {
    let output = run_cli(&["1 + /* oops"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: unterminated block comment\n");
}

#[test]
fn evaluates_comparison_program_and_prints_boolean_result() {
    let output = run_cli(&["let ready = 3 >= 2; ready"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"true\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn reports_comparison_results_used_in_arithmetic_as_a_type_error() {
    let output = run_cli(&["let ready = 3 >= 2; ready * 10"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: type mismatch in '*': expected two integers, found boolean and integer\n"
    );
}

#[test]
fn evaluates_boolean_programs_and_prints_true_or_false() {
    let output = run_cli(&["1 < 2 && 3 < 4"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"true\n");
    assert!(output.stderr.is_empty());

    let output = run_cli(&["true && false || true"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"true\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_string_programs_and_prints_without_quotes() {
    let output = run_cli(&["\"hello\" + \" \" + \"world\""]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"hello world\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_function_programs() {
    let output =
        run_cli(&["fn double(x) = { x * 2 }; fn add(a, b) = { a + b }; double(20) + add(1, 1)"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_recursive_function_programs() {
    let output = run_cli(&[
        "fn factorial(n) = { if n <= 1 { 1 } else { n * factorial(n - 1) } }; factorial(6)",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"720\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn reports_undefined_functions_as_a_type_error() {
    let output = run_cli(&["fn f(x) = { x + 1 }; missing(1)"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("undefined function: 'missing'"));
}

#[test]
fn evaluates_if_else_programs() {
    let output = run_cli(&["if 1 < 2 { 10 } else { 20 }"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"10\n");
    assert!(output.stderr.is_empty());

    let output = run_cli(&["if 1 > 2 { \"hot\" } else { \"cold\" }"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"cold\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn reports_type_errors_with_positions() {
    let output = run_cli(&["--positions", "1 + true"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: type mismatch in '+': expected two integers or two strings, found integer and boolean\n at line 1, column 3\n"
    );
}

#[test]
fn reports_unterminated_string_literals() {
    let output = run_cli(&["\"oops"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: unterminated string literal\n");
}

#[test]
fn rejects_chained_comparisons() {
    let output = run_cli(&["1 < 2 < 3"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: comparison operators cannot be chained\n"
    );
}

#[test]
fn evaluates_programs_with_modulo() {
    let output = run_cli(&["10 % 3"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"1\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_modulo_program_from_a_file() {
    let source = TemporarySource::new(b"let a = 10; a % 3");
    let output = run_cli(&["--file", source.as_str()]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"1\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn reports_modulo_by_zero_on_stderr() {
    let output = run_cli(&["8 % (3 - 3)"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: division by zero\n");
}

#[test]
fn reports_remainder_overflow_on_stderr() {
    let output = run_cli(&["-9223372036854775808 % -1"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: integer remainder overflow\n");
}

#[test]
fn positions_flag_reports_modulo_error_position() {
    let output = run_cli(&["--positions", "8 % (3 - 3)"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: division by zero\n at line 1, column 3\n"
    );
}

#[test]
fn reports_invalid_expression_on_stderr_without_stdout() {
    let output = run_cli(&["8 / (3 - 3)"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: division by zero\n");
}

#[test]
fn reports_undefined_variable_on_stderr_without_stdout() {
    let output = run_cli(&["let result = missing + 1; result"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: undefined variable: 'missing'\n");
}

#[test]
fn preserves_evaluation_errors_for_file_input() {
    let source = TemporarySource::new(b"8 / (3 - 3)");
    let output = run_cli(&["--file", source.as_str()]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: division by zero\n");
}

#[test]
fn reports_missing_expression_with_failure_status() {
    let output = run_cli(&[]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: expected exactly one expression argument\n"
    );
}

#[test]
fn reports_missing_file_path() {
    for flag in ["-f", "--file"] {
        let output = run_cli(&[flag]);

        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"error: missing file path after -f/--file\n");
    }
}

#[test]
fn rejects_conflicting_source_modes() {
    let source = TemporarySource::new(b"1");

    let file_then_stdin = run_cli(&["--file", source.as_str(), "--stdin"]);
    assert!(!file_then_stdin.status.success());
    assert!(file_then_stdin.stdout.is_empty());
    assert_eq!(
        file_then_stdin.stderr,
        b"error: -f/--file accepts exactly one path and cannot be combined with additional arguments\n"
    );

    let stdin_then_file = run_cli(&["--stdin", "--file", source.as_str()]);
    assert!(!stdin_then_file.status.success());
    assert!(stdin_then_file.stdout.is_empty());
    assert_eq!(
        stdin_then_file.stderr,
        b"error: --stdin cannot be combined with additional arguments\n"
    );
}

#[test]
fn rejects_additional_arguments_after_file_input() {
    let source = TemporarySource::new(b"1");
    let output = run_cli(&["--file", source.as_str(), "extra"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: -f/--file accepts exactly one path and cannot be combined with additional arguments\n"
    );
}

#[test]
fn rejects_additional_arguments_after_standard_input() {
    let output = run_cli_with_stdin(&["--stdin", "extra"], b"1");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: --stdin cannot be combined with additional arguments\n"
    );
}

#[test]
fn reports_unreadable_file_with_path_and_io_reason() {
    let source = TemporarySource::new(b"1");
    let missing_path = format!("{}.missing", source.as_str());
    let output = run_cli(&["--file", missing_path.as_str()]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to read source file '"));
    assert!(stderr.contains(".missing': "));
}

#[test]
fn reports_invalid_utf8_file_with_source_context() {
    let source = TemporarySource::new(&[b'1', b' ', 0xff]);
    let output = run_cli(&["--file", source.as_str()]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr should be UTF-8");
    assert!(stderr.starts_with("error: source file '"));
    assert!(stderr.contains("' is not valid UTF-8: "));
}

#[test]
fn reports_invalid_utf8_standard_input_with_source_context() {
    let output = run_cli_with_stdin(&["--stdin"], &[b'1', b' ', 0xff]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr should be UTF-8");
    assert_eq!(
        stderr,
        "error: standard input is not valid UTF-8: invalid utf-8 sequence of 1 bytes from index 2\n"
    );
}

fn expected_help() -> String {
    concat!(
        "Usage: rusty-buggy-language \"<program>\"\n",
        "       rusty-buggy-language -f <path> | --file <path>\n",
        "       rusty-buggy-language --stdin\n",
        "       rusty-buggy-language --repl\n",
        "       rusty-buggy-language -h | --help\n",
        "       rusty-buggy-language -V | --version\n",
        "       rusty-buggy-language [--positions] [--input-limit <bytes>] <program>\n",
        "       rusty-buggy-language [--positions] [--input-limit <bytes>] -f <path> | --file <path>\n",
        "       rusty-buggy-language [--positions] [--input-limit <bytes>] --stdin\n",
        "\n",
        "Evaluates an expression program with immutable let bindings, integers, booleans (true/false, !, &&, ||), strings (\"...\" with \\n, \\t, \\\\, and \\\" escapes), if/else expressions with { } blocks, function declarations (fn name(param, ...) = { body }; with recursive calls), built-in functions (len, int_to_string, string_to_int, bool_to_int, int_to_bool), comparisons (<, <=, >, >=, ==, !=; <, <=, >, >= also order strings), +, -, *, /, %, // and /* */ comments, parentheses, and prefix -.\n",
        "\n",
        "The program can be supplied inline, read as UTF-8 from a file, or read as UTF-8 from standard input. Source modes are mutually exclusive. `--repl` reads one program per line from standard input and prints each result.\n",
        "\n",
        "--positions      Also report the line and column of evaluation or syntax errors.\n",
        "--input-limit N  Reject programs longer than N bytes before evaluation.\n",
    )
    .to_string()
}

#[test]
fn prints_help_for_short_flag() {
    let output = run_cli(&["-h"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, expected_help().as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn prints_help_for_long_flag() {
    let output = run_cli(&["--help"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, expected_help().as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn rejects_help_flag_with_extra_arguments() {
    let output = run_cli(&["--help", "1"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: expected exactly one expression argument\n"
    );
}

#[test]
fn prints_version_for_short_flag() {
    let output = run_cli(&["-V"]);

    assert!(output.status.success());
    let expected = format!("rusty-buggy-language {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(output.stdout, expected.as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn prints_version_for_long_flag() {
    let output = run_cli(&["--version"]);

    assert!(output.status.success());
    let expected = format!("rusty-buggy-language {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(output.stdout, expected.as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn rejects_short_version_flag_with_extra_arguments() {
    let output = run_cli(&["-V", "1"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: expected exactly one expression argument\n"
    );
}

#[test]
fn rejects_long_version_flag_with_extra_arguments() {
    let output = run_cli(&["--version", "1"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: expected exactly one expression argument\n"
    );
}

#[test]
fn positions_flag_keeps_default_output_for_successful_programs() {
    let output = run_cli(&["--positions", "1 + 2"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn positions_flag_reports_line_and_column_for_evaluation_errors() {
    let output = run_cli(&["--positions", "8 / (3 - 3)"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: division by zero\n at line 1, column 3\n"
    );
}

#[test]
fn without_positions_error_output_is_unchanged() {
    let output = run_cli(&["8 / (3 - 3)"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: division by zero\n");
}

#[test]
fn positions_flag_reports_line_and_column_after_newlines() {
    let output = run_cli(&["--positions", "let x = 1;\n missing + 1"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: undefined variable: 'missing'\n at line 2, column 2\n"
    );
}

#[test]
fn input_limit_flag_rejects_oversized_inline_programs() {
    let output = run_cli(&["--input-limit", "5", "1 + 2 + 3"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: program is too large to evaluate\n");
}

#[test]
fn input_limit_flag_accepts_programs_at_the_limit() {
    // "1 + 2 + 3" is exactly 9 bytes.
    let output = run_cli(&["--input-limit", "9", "1 + 2 + 3"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"6\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn input_limit_flag_rejects_non_numeric_values() {
    let output = run_cli(&["--input-limit", "lots", "1"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: invalid --input-limit value: 'lots'\n"
    );
}

#[test]
fn positions_and_input_limit_flags_can_be_combined() {
    let output = run_cli(&["--positions", "--input-limit", "5", "1 + 2"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn input_limit_flag_applies_to_file_sources() {
    let source = TemporarySource::new(b"1 + 2 + 3");
    let output = run_cli(&["--input-limit", "5", "--file", source.as_str()]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"error: program is too large to evaluate\n");
}

#[test]
fn repl_evaluates_one_program_per_line() {
    let output = run_cli_with_stdin(&["--repl"], b"1 + 2 * 3\n8 / (3 - 3)\nlet x = 4; x * 5\n");

    assert!(output.status.success());
    assert_eq!(output.stdout, b">\n7\n>\nerror: division by zero\n>\n20\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn repl_requires_it_as_the_sole_argument() {
    let output = run_cli(&["--repl", "1"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"error: expected exactly one expression argument\n"
    );
}

#[test]
fn repl_fails_when_no_programs_are_supplied() {
    let output = run_cli_with_stdin(&["--repl"], b"\n\n");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b">\n>\n");
    assert!(output.stderr.is_empty());
}
