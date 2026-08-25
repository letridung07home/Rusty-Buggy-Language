use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

use rusty_buggy_language::evaluate;

const HELP: &str = "Usage: rusty-buggy-language \"<program>\"\n       rusty-buggy-language -f <path> | --file <path>\n       rusty-buggy-language --stdin\n       rusty-buggy-language -h | --help\n       rusty-buggy-language -V | --version\n\nEvaluates an i64 integer program with immutable let bindings, comparisons (<, <=, >, >=, ==, !=), +, -, *, /, parentheses, and prefix -.\n\nThe program can be supplied inline, read as UTF-8 from a file, or read as UTF-8 from standard input.";
const VERSION: &str = concat!("rusty-buggy-language ", env!("CARGO_PKG_VERSION"));

pub(super) fn run<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    match execute(args) {
        Ok(Output::Value(value)) => println!("{value}"),
        Ok(Output::Help) => println!("{HELP}"),
        Ok(Output::Version) => println!("{VERSION}"),
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

#[derive(Debug, PartialEq)]
enum Output {
    Value(i64),
    Help,
    Version,
}

fn execute<I>(args: I) -> Result<Output, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut stdin = io::stdin();
    execute_with_reader(args, &mut stdin)
}

fn execute_with_reader<I, R>(args: I, reader: &mut R) -> Result<Output, String>
where
    I: IntoIterator<Item = OsString>,
    R: Read,
{
    let mut args = args.into_iter();
    let first_argument = args
        .next()
        .ok_or_else(|| "expected exactly one expression argument".to_owned())?;

    if first_argument == "-h" || first_argument == "--help" {
        return if args.next().is_none() {
            Ok(Output::Help)
        } else {
            Err("expected exactly one expression argument".to_owned())
        };
    }

    if first_argument == "-V" || first_argument == "--version" {
        return if args.next().is_none() {
            Ok(Output::Version)
        } else {
            Err("expected exactly one expression argument".to_owned())
        };
    }

    if first_argument == "-f" || first_argument == "--file" {
        let path = args
            .next()
            .ok_or_else(|| "missing file path after -f/--file".to_owned())?;

        if args.next().is_some() {
            return Err(
                "-f/--file accepts exactly one path and cannot be combined with additional arguments"
                    .to_owned(),
            );
        }

        return read_file(&path).and_then(evaluate_source);
    }

    if first_argument == "--stdin" {
        if args.next().is_some() {
            return Err("--stdin cannot be combined with additional arguments".to_owned());
        }

        return read_stdin(reader).and_then(evaluate_source);
    }

    if args.next().is_some() {
        return Err("expected exactly one expression argument".to_owned());
    }

    let expression = first_argument
        .into_string()
        .map_err(|_| "expression must be valid UTF-8".to_owned())?;

    evaluate_source(expression)
}

fn evaluate_source(source: String) -> Result<Output, String> {
    evaluate(&source)
        .map(Output::Value)
        .map_err(|error| error.to_string())
}

fn read_file(path: &OsString) -> Result<String, String> {
    let path_display = Path::new(path).display();
    let bytes = fs::read(path).map_err(|error| {
        format!("failed to read source file '{path_display}': {error}")
    })?;

    String::from_utf8(bytes).map_err(|error| {
        format!("source file '{path_display}' is not valid UTF-8: {error}")
    })
}

fn read_stdin<R>(reader: &mut R) -> Result<String, String>
where
    R: Read,
{
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read standard input: {error}"))?;

    String::from_utf8(bytes)
        .map_err(|error| format!("standard input is not valid UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{execute, Output};
    use std::ffi::OsString;

    fn arguments(arguments: &[&str]) -> Vec<OsString> {
        arguments.iter().map(OsString::from).collect()
    }

    #[test]
    fn missing_expression_is_an_argument_error() {
        assert_eq!(
            execute(arguments(&[])),
            Err("expected exactly one expression argument".to_owned())
        );
    }

    #[test]
    fn extra_arguments_are_rejected() {
        assert_eq!(
            execute(arguments(&["1", "2"])),
            Err("expected exactly one expression argument".to_owned())
        );
    }

    #[test]
    fn short_help_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(execute(arguments(&["-h"])), Ok(Output::Help));
    }

    #[test]
    fn long_help_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(execute(arguments(&["--help"])), Ok(Output::Help));
    }

    #[test]
    fn short_version_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(execute(arguments(&["-V"])), Ok(Output::Version));
    }

    #[test]
    fn long_version_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(execute(arguments(&["--version"])), Ok(Output::Version));
    }

    #[test]
    fn help_with_extra_arguments_is_rejected() {
        assert_eq!(
            execute(arguments(&["--help", "1"])),
            Err("expected exactly one expression argument".to_owned())
        );
    }

    #[test]
    fn version_with_extra_arguments_is_rejected() {
        for flag in ["-V", "--version"] {
            assert_eq!(
                execute(arguments(&[flag, "1"])),
                Err("expected exactly one expression argument".to_owned())
            );
        }
    }

    #[test]
    fn file_mode_requires_a_path() {
        for flag in ["-f", "--file"] {
            assert_eq!(
                execute(arguments(&[flag])),
                Err("missing file path after -f/--file".to_owned())
            );
        }
    }

    #[test]
    fn file_mode_rejects_additional_arguments() {
        assert_eq!(
            execute(arguments(&["--file", "program.rbl", "--stdin"])),
            Err("-f/--file accepts exactly one path and cannot be combined with additional arguments".to_owned())
        );
    }

    #[test]
    fn stdin_mode_rejects_additional_arguments() {
        assert_eq!(
            execute(arguments(&["--stdin", "program"])),
            Err("--stdin cannot be combined with additional arguments".to_owned())
        );
    }

    #[test]
    fn stdin_mode_reads_the_complete_source() {
        let mut input = std::io::Cursor::new(
            b"let first = 2;\nlet second = first + 3;\nsecond * 4".to_vec(),
        );

        assert_eq!(
            execute_with_reader(arguments(&["--stdin"]), &mut input),
            Ok(Output::Value(20))
        );
    }

    #[test]
    fn stdin_mode_reports_invalid_utf8_with_source_context() {
        let mut input = std::io::Cursor::new(vec![b'1', b' ', 0xff]);

        assert_eq!(
            execute_with_reader(arguments(&["--stdin"]), &mut input),
            Err(
                "standard input is not valid UTF-8: invalid utf-8 sequence of 1 bytes from index 2"
                    .to_owned(),
            )
        );
    }
}
