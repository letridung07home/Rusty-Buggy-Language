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
fn reports_invalid_expression_on_stderr_without_stdout() {
    let output = run_cli(&["8 / (3 - 3)"]);

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
fn prints_help_for_short_flag() {
    let output = run_cli(&["-h"]);

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"Usage: rusty-buggy-language \"<expression>\"\n\nEvaluates an i64 integer expression with +, -, *, /, parentheses, and prefix -.\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn prints_help_for_long_flag() {
    let output = run_cli(&["--help"]);

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"Usage: rusty-buggy-language \"<expression>\"\n\nEvaluates an i64 integer expression with +, -, *, /, parentheses, and prefix -.\n"
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
