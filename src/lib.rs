mod ast;
mod error;
mod evaluator;
mod lexer;
mod parser;

pub use error::{Error, SourcePosition};

/// The default maximum input size (in bytes) enforced by [`evaluate`].
///
/// This is generous enough for any realistic integer program while bounding
/// the memory and evaluation time a single adversarial program can consume.
/// Callers needing a different bound can use [`evaluate_with_limits`].
pub const DEFAULT_MAX_INPUT_BYTES: usize = 1_048_576; // 1 MiB

/// Resource limits applied when evaluating a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum accepted program size in bytes. Programs longer than this are
    /// rejected before parsing.
    pub max_input_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
        }
    }
}

/// Evaluates a Rusty Buggy Language program with the default [`Limits`].
pub fn evaluate(program: &str) -> Result<i64, Error> {
    evaluate_with_limits(program, &Limits::default())
}

/// Evaluates a Rusty Buggy Language program under the given resource [`Limits`].
pub fn evaluate_with_limits(program: &str, limits: &Limits) -> Result<i64, Error> {
    if program.len() > limits.max_input_bytes {
        return Err(Error::new("program is too large to evaluate"));
    }

    let tokens = lexer::Lexer::new(program).tokenize()?;
    let program = parser::Parser::new(&tokens).parse()?;
    evaluator::evaluate(&program)
}

#[cfg(test)]
mod tests {
    use super::{evaluate, evaluate_with_limits, Limits, DEFAULT_MAX_INPUT_BYTES};

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

    #[test]
    fn exposes_the_position_of_an_evaluation_error() {
        // The '/' operator sits at line 1, column 3.
        let error = evaluate("8 / (3 - 3)").unwrap_err();

        assert_eq!(
            error.position(),
            Some(super::SourcePosition { line: 1, column: 3 })
        );
    }

    #[test]
    fn rejects_input_larger_than_the_limit() {
        let limits = Limits { max_input_bytes: 5 };

        assert_eq!(
            evaluate_with_limits("1 + 2 + 3 + 4", &limits)
                .unwrap_err()
                .to_string(),
            "program is too large to evaluate"
        );
    }

    #[test]
    fn accepts_input_at_the_limit_boundary() {
        let limits = Limits { max_input_bytes: 9 };

        // "1 + 2 + 3" is exactly 9 bytes.
        assert_eq!(evaluate_with_limits("1 + 2 + 3", &limits), Ok(6));
    }

    #[test]
    fn default_evaluate_uses_the_configured_default_limit() {
        // The default limit is far above realistic programs, so a normal
        // program evaluates successfully.
        assert_eq!(evaluate("1 + 2"), Ok(3));
    }

    #[test]
    fn limits_do_not_change_semantics_when_within_the_bound() {
        let limits = Limits {
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
        };

        assert_eq!(
            evaluate_with_limits("let a = 10; let b = a * 3; b - 2", &limits),
            Ok(28)
        );
    }

    #[test]
    fn comments_are_stripped_before_evaluation() {
        assert_eq!(evaluate("1 /* part */ + 2 // note"), Ok(3));
        assert_eq!(evaluate("let rate = 20; // per hour\nrate * 5"), Ok(100));
        assert_eq!(evaluate("let a = 1; /* multi\nline */ a + 1"), Ok(2));
    }

    #[test]
    fn comments_do_not_shift_error_positions() {
        // The '/' operator sits at line 2, column 3 of the commented program.
        let error = evaluate("// note\n8 / (3 - 3)").unwrap_err();

        assert_eq!(error.to_string(), "division by zero");
        assert_eq!(
            error.position(),
            Some(super::SourcePosition { line: 2, column: 3 })
        );
    }

    #[test]
    fn a_comment_only_program_is_empty() {
        assert_eq!(
            evaluate("// just a comment").unwrap_err().to_string(),
            "expression is empty"
        );
        assert_eq!(
            evaluate("/* nothing else */").unwrap_err().to_string(),
            "expression is empty"
        );
    }
}
