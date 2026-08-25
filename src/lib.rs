mod ast;
mod error;
mod evaluator;
mod lexer;
mod parser;

pub use error::Error;

/// Evaluates a Rusty Buggy Language program.
pub fn evaluate(program: &str) -> Result<i64, Error> {
    let tokens = lexer::Lexer::new(program).tokenize()?;
    let program = parser::Parser::new(&tokens).parse()?;
    evaluator::evaluate(&program)
}

#[cfg(test)]
mod tests {
    use super::evaluate;

    #[test]
    fn evaluates_program_through_the_library_facade() {
        assert_eq!(
            evaluate("let rate = 20; let quantity = 5; rate * quantity"),
            Ok(100)
        );
    }

    #[test]
    fn exposes_the_existing_error_message_through_display() {
        let error = evaluate("8 / (3 - 3)").unwrap_err();

        assert_eq!(error.to_string(), "division by zero");
    }
}
