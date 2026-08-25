use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

use rusty_buggy_language::{evaluate_with_limits, Error, Limits};

const HELP: &str = "Usage: rusty-buggy-language \"<program>\"\n       rusty-buggy-language -f <path> | --file <path>\n       rusty-buggy-language --stdin\n       rusty-buggy-language -h | --help\n       rusty-buggy-language -V | --version\n       rusty-buggy-language [--positions] [--input-limit <bytes>] <program>\n       rusty-buggy-language [--positions] [--input-limit <bytes>] -f <path> | --file <path>\n       rusty-buggy-language [--positions] [--input-limit <bytes>] --stdin\n\nEvaluates an i64 integer program with immutable let bindings, comparisons (<, <=, >, >=, ==, !=), +, -, *, /, parentheses, and prefix -.\n\nThe program can be supplied inline, read as UTF-8 from a file, or read as UTF-8 from standard input. Source modes are mutually exclusive.\n\n--positions      Also report the line and column of evaluation or syntax errors.\n--input-limit N  Reject programs longer than N bytes before evaluation.";

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
    let args: Vec<OsString> = args.into_iter().collect();

    if args.len() == 1 && (args[0] == "-h" || args[0] == "--help") {
        return Ok(Output::Help);
    }

    if args.len() == 1 && (args[0] == "-V" || args[0] == "--version") {
        return Ok(Output::Version);
    }

    let invocation = parse_invocation(&args)?;

    let source = invocation
        .source
        .ok_or_else(|| "expected exactly one expression argument".to_owned())?;

    let source = read_source(source, reader)?;
    let limits = match invocation.input_limit {
        Some(max_input_bytes) => Limits { max_input_bytes },
        None => Limits::default(),
    };

    evaluate_source(source, &limits, invocation.positions)
}

fn read_source<R>(source: SourceArg, reader: &mut R) -> Result<String, String>
where
    R: Read,
{
    match source {
        SourceArg::Inline(expression) => Ok(expression),
        SourceArg::File(path) => read_file(&path),
        SourceArg::Stdin => read_stdin(reader),
    }
}

fn evaluate_source(source: String, limits: &Limits, positions: bool) -> Result<Output, String> {
    match evaluate_with_limits(&source, limits) {
        Ok(value) => Ok(Output::Value(value)),
        Err(error) => Err(format_error(&error, positions)),
    }
}

fn format_error(error: &Error, positions: bool) -> String {
    if !positions {
        return error.message().to_owned();
    }

    match error.position() {
        Some(position) => format!(
            "{}\n at line {}, column {}",
            error.message(),
            position.line,
            position.column
        ),
        None => error.message().to_owned(),
    }
}

/// The message reported when a conflict occurs after `source` was already the
/// first source the user selected.
fn conflict_message(source: &SourceArg) -> &'static str {
    match source {
        SourceArg::Inline(_) => "expected exactly one expression argument",
        SourceArg::File(_) => {
            "-f/--file accepts exactly one path and cannot be combined with additional arguments"
        }
        SourceArg::Stdin => "--stdin cannot be combined with additional arguments",
    }
}

#[derive(Debug, PartialEq)]
enum SourceArg {
    Inline(String),
    File(OsString),
    Stdin,
}

struct Invocation {
    source: Option<SourceArg>,
    input_limit: Option<usize>,
    positions: bool,
}

fn parse_invocation(args: &[OsString]) -> Result<Invocation, String> {
    let mut invocation = Invocation {
        source: None,
        input_limit: None,
        positions: false,
    };

    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        let as_str = argument.to_str();

        if as_str == Some("--positions") {
            invocation.positions = true;
            index += 1;
            continue;
        }

        if as_str == Some("--input-limit") {
            let value = args.get(index + 1).ok_or_else(|| {
                "missing value after --input-limit".to_owned()
            })?;
            let value_str = value
                .to_str()
                .ok_or_else(|| "--input-limit value must be valid UTF-8".to_owned())?;
            let bytes = value_str
                .parse::<usize>()
                .map_err(|_| format!("invalid --input-limit value: '{value_str}'"))?;
            invocation.input_limit = Some(bytes);
            index += 2;
            continue;
        }

        if as_str == Some("-f") || as_str == Some("--file") {
            let path = args
                .get(index + 1)
                .ok_or_else(|| "missing file path after -f/--file".to_owned())?
                .clone();
            if let Some(source) = &invocation.source {
                return Err(conflict_message(source).to_owned());
            }
            invocation.source = Some(SourceArg::File(path));
            index += 2;
            continue;
        }

        if as_str == Some("--stdin") {
            if let Some(source) = &invocation.source {
                return Err(conflict_message(source).to_owned());
            }
            invocation.source = Some(SourceArg::Stdin);
            index += 1;
            continue;
        }

        // Inline program argument.
        let expression = argument
            .clone()
            .into_string()
            .map_err(|_| "expression must be valid UTF-8".to_owned())?;
        if let Some(source) = &invocation.source {
            return Err(conflict_message(source).to_owned());
        }
        invocation.source = Some(SourceArg::Inline(expression));
        index += 1;
    }

    Ok(invocation)
}

fn read_file(path: &OsString) -> Result<String, String> {
    let path_display = Path::new(path).display();
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read source file '{path_display}': {error}"))?;

    String::from_utf8(bytes)
        .map_err(|error| format!("source file '{path_display}' is not valid UTF-8: {error}"))
}

fn read_stdin<R>(reader: &mut R) -> Result<String, String>
where
    R: Read,
{
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read standard input: {error}"))?;

    String::from_utf8(bytes).map_err(|error| format!("standard input is not valid UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{execute, execute_with_reader, Output};
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
        let mut input =
            std::io::Cursor::new(b"let first = 2;\nlet second = first + 3;\nsecond * 4".to_vec());

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

    #[test]
    fn positions_flag_is_allowed_alongside_an_inline_program() {
        assert_eq!(
            execute(arguments(&["--positions", "1 + 2"])),
            Ok(Output::Value(3))
        );
    }

    #[test]
    fn input_limit_flag_appends_its_value_to_an_inline_program() {
        assert_eq!(
            execute(arguments(&["--input-limit", "5", "1 + 2"])),
            Ok(Output::Value(3))
        );
    }

    #[test]
    fn input_limit_flag_rejects_oversized_inline_programs() {
        assert_eq!(
            execute(arguments(&["--input-limit", "5", "1 + 2 + 3"])),
            Err("program is too large to evaluate".to_owned())
        );
    }

    #[test]
    fn positions_flag_augments_the_error_output() {
        assert_eq!(
            execute(arguments(&["--positions", "1 + 2 3"])),
            Err("unexpected trailing token: integer literal\n at line 1, column 7".to_owned())
        );
    }
}