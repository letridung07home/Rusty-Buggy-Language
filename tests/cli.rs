use std::process::{Command, Output};

fn run_cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rusty-buggy-language"))
        .args(arguments)
        .output()
        .expect("failed to execute the compiled CLI")
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
fn prints_help_for_short_flag() {
    let output = run_cli(&["-h"]);

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"Usage: rusty-buggy-language \"<program>\"\n       rusty-buggy-language -h | --help\n       rusty-buggy-language -V | --version\n\nEvaluates an i64 integer program with immutable let bindings, +, -, *, /, parentheses, and prefix -.\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn prints_help_for_long_flag() {
    let output = run_cli(&["--help"]);

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"Usage: rusty-buggy-language \"<program>\"\n       rusty-buggy-language -h | --help\n       rusty-buggy-language -V | --version\n\nEvaluates an i64 integer program with immutable let bindings, +, -, *, /, parentheses, and prefix -.\n"
    );
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
    assert_eq!(output.stdout, b"rusty-buggy-language 0.5.1\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn prints_version_for_long_flag() {
    let output = run_cli(&["--version"]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"rusty-buggy-language 0.5.1\n");
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
