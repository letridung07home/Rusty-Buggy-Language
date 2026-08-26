use std::fmt;

/// A 1-based line and column in the source program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    /// 1-based line number in the source program.
    pub line: usize,
    /// 1-based column number (in characters) on the line.
    pub column: usize,
}

/// An evaluation or syntax error, carrying a stable user-facing message and
/// an optional source [`position`](Error::position).
#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    message: String,
    position: Option<SourcePosition>,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: None,
        }
    }

    pub(crate) fn at(message: impl Into<String>, position: SourcePosition) -> Self {
        Self {
            message: message.into(),
            position: Some(position),
        }
    }

    /// The message text, rendered by [`std::fmt::Display`].
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The 1-based source position associated with this error, if any.
    pub fn position(&self) -> Option<SourcePosition> {
        self.position
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
