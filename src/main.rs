mod expression;

use std::process::ExitCode;

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(value) => println!("{value}"),
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

fn run<I>(args: I) -> Result<i64, String>
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

    let expression = expression
        .into_string()
        .map_err(|_| "expression must be valid UTF-8".to_owned())?;

    expression::evaluate(&expression)
}

#[cfg(test)]
mod tests {
    use super::run;
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
}
