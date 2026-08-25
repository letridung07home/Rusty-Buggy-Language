mod expression;

use std::process::ExitCode;

const HELP: &str = "Usage: rusty-buggy-language \"<expression>\"\n       rusty-buggy-language -h | --help\n       rusty-buggy-language -V | --version\n\nEvaluates an i64 integer expression with +, -, *, /, parentheses, and prefix -.";
const VERSION: &str = concat!("rusty-buggy-language ", env!("CARGO_PKG_VERSION"));

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
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

fn run<I>(args: I) -> Result<Output, String>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut args = args.into_iter();
    let expression = args
        .next()
        .ok_or_else(|| "expected exactly one expression argument".to_owned())?;

    if args.next().is_some() {
        return Err("expected exactly one expression argument".to_owned());
    }

    if expression == "-h" || expression == "--help" {
        return Ok(Output::Help);
    }

    if expression == "-V" || expression == "--version" {
        return Ok(Output::Version);
    }

    let expression = expression
        .into_string()
        .map_err(|_| "expression must be valid UTF-8".to_owned())?;

    expression::evaluate(&expression).map(Output::Value)
}

#[cfg(test)]
mod tests {
    use super::{run, Output};
    use std::ffi::OsString;

    fn arguments(arguments: &[&str]) -> Vec<OsString> {
        arguments.iter().map(OsString::from).collect()
    }

    #[test]
    fn missing_expression_is_an_argument_error() {
        assert_eq!(
            run(arguments(&[])),
            Err("expected exactly one expression argument".to_owned())
        );
    }

    #[test]
    fn extra_arguments_are_rejected() {
        assert_eq!(
            run(arguments(&["1", "2"])),
            Err("expected exactly one expression argument".to_owned())
        );
    }

    #[test]
    fn short_help_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(run(arguments(&["-h"])), Ok(Output::Help));
    }

    #[test]
    fn long_help_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(run(arguments(&["--help"])), Ok(Output::Help));
    }

    #[test]
    fn short_version_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(run(arguments(&["-V"])), Ok(Output::Version));
    }

    #[test]
    fn long_version_is_recognized_when_it_is_the_only_argument() {
        assert_eq!(run(arguments(&["--version"])), Ok(Output::Version));
    }

    #[test]
    fn help_with_extra_arguments_is_rejected() {
        assert_eq!(
            run(arguments(&["--help", "1"])),
            Err("expected exactly one expression argument".to_owned())
        );
    }

    #[test]
    fn version_with_extra_arguments_is_rejected() {
        for flag in ["-V", "--version"] {
            assert_eq!(
                run(arguments(&[flag, "1"])),
                Err("expected exactly one expression argument".to_owned())
            );
        }
    }
}
