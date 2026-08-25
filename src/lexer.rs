use crate::error::{Error, SourcePosition};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) position: SourcePosition,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Integer(u64),
    Identifier(String),
    Let,
    Equals,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    EqualEqual,
    NotEqual,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
}

pub(crate) struct Lexer<'a> {
    input: &'a str,
    position: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            line: 1,
            column: 1,
        }
    }

    pub(crate) fn tokenize(mut self) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        while let Some(character) = self.current_character() {
            if character.is_ascii_whitespace() {
                self.advance();
                continue;
            }

            if character.is_ascii_digit() {
                tokens.push(self.integer_literal()?);
                continue;
            }

            if character.is_ascii_alphabetic() || character == '_' {
                tokens.push(self.identifier());
                continue;
            }

            let position = self.current_position();
            let kind = match character {
                '=' => self.operator_with_optional_equals(TokenKind::Equals, TokenKind::EqualEqual),
                '<' => self.operator_with_optional_equals(
                    TokenKind::LessThan,
                    TokenKind::LessThanOrEqual,
                ),
                '>' => self.operator_with_optional_equals(
                    TokenKind::GreaterThan,
                    TokenKind::GreaterThanOrEqual,
                ),
                '!' => {
                    self.advance();
                    if self.current_character() == Some('=') {
                        self.advance();
                        TokenKind::NotEqual
                    } else {
                        return Err(self.error_at_current("unexpected character '!'"));
                    }
                }
                ';' => {
                    self.advance();
                    TokenKind::Semicolon
                }
                '+' => {
                    self.advance();
                    TokenKind::Plus
                }
                '-' => {
                    self.advance();
                    TokenKind::Minus
                }
                '*' => {
                    self.advance();
                    TokenKind::Star
                }
                '/' => {
                    self.advance();
                    TokenKind::Slash
                }
                '(' => {
                    self.advance();
                    TokenKind::LeftParen
                }
                ')' => {
                    self.advance();
                    TokenKind::RightParen
                }
                _ => return Err(self.error_at_current(format!("unexpected character '{character}'"))),
            };
            tokens.push(Token { kind, position });
        }

        Ok(tokens)
    }

    fn error_at_current(&self, message: impl Into<String>) -> Error {
        Error::at(message, self.current_position())
    }

    fn current_position(&self) -> SourcePosition {
        SourcePosition {
            line: self.line,
            column: self.column,
        }
    }

    fn operator_with_optional_equals(&mut self, single: TokenKind, compound: TokenKind) -> TokenKind {
        self.advance();
        if self.current_character() == Some('=') {
            self.advance();
            compound
        } else {
            single
        }
    }

    fn integer_literal(&mut self) -> Result<Token, Error> {
        let position = self.current_position();
        let mut value = 0_u64;

        while let Some(character) = self.current_character() {
            if !character.is_ascii_digit() {
                break;
            }

            let digit = u64::from(character as u8 - b'0');
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(|| self.error_at_current("integer literal out of range"))?;
            self.advance();
        }

        Ok(Token {
            kind: TokenKind::Integer(value),
            position,
        })
    }

    fn identifier(&mut self) -> Token {
        let start = self.position;
        let position = self.current_position();

        while let Some(character) = self.current_character() {
            if !character.is_ascii_alphanumeric() && character != '_' {
                break;
            }
            self.advance();
        }

        let name = &self.input[start..self.position];
        let kind = if name == "let" {
            TokenKind::Let
        } else {
            TokenKind::Identifier(name.to_owned())
        };

        Token { kind, position }
    }

    fn current_character(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(character) = self.current_character() {
            self.position += character.len_utf8();
            if character == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }
}

impl Token {
    pub(crate) fn kind_name(&self) -> &'static str {
        match &self.kind {
            TokenKind::Integer(_) => "integer literal",
            TokenKind::Identifier(_) => "identifier",
            TokenKind::Let => "'let'",
            TokenKind::Equals => "'='",
            TokenKind::LessThan => "'<'",
            TokenKind::LessThanOrEqual => "'<='",
            TokenKind::GreaterThan => "'>'",
            TokenKind::GreaterThanOrEqual => "'>='",
            TokenKind::EqualEqual => "'=='",
            TokenKind::NotEqual => "'!='",
            TokenKind::Semicolon => "';'",
            TokenKind::Plus => "'+'",
            TokenKind::Minus => "'-'",
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::LeftParen => "'('",
            TokenKind::RightParen => "')'",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use crate::error::SourcePosition;
    use crate::lexer::TokenKind;

    #[test]
    fn tokenizes_the_language_symbols_and_identifiers() {
        let tokens = Lexer::new("let _value2 = 1 <= 2 != 3;").tokenize();
        let kinds: Vec<TokenKind> = tokens
            .as_ref()
            .unwrap()
            .iter()
            .map(|token| token.kind.clone())
            .collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Let,
                TokenKind::Identifier("_value2".to_owned()),
                TokenKind::Equals,
                TokenKind::Integer(1),
                TokenKind::LessThanOrEqual,
                TokenKind::Integer(2),
                TokenKind::NotEqual,
                TokenKind::Integer(3),
                TokenKind::Semicolon,
            ]
        );
    }

    #[test]
    fn tracks_line_and_column_positions_across_multiple_lines() {
        let tokens = Lexer::new("let a = 1;\n  a + 2").tokenize().unwrap();

        // 'let' at line 1, column 1
        assert_eq!(tokens[0].position, SourcePosition { line: 1, column: 1 });
        // 'a' at line 1, column 5
        assert_eq!(tokens[1].position, SourcePosition { line: 1, column: 5 });
        // 'a' after newline at line 2, column 3
        assert_eq!(tokens[6].position, SourcePosition { line: 2, column: 3 });
        // '+' at line 2, column 5
        assert_eq!(tokens[7].position, SourcePosition { line: 2, column: 5 });
        // '2' at line 2, column 7
        assert_eq!(tokens[8].position, SourcePosition { line: 2, column: 7 });
    }

    #[test]
    fn rejects_invalid_characters() {
        assert_eq!(
            Lexer::new("1 @ 2").tokenize().unwrap_err().to_string(),
            "unexpected character '@'"
        );
    }

    #[test]
    fn reports_the_position_of_an_invalid_character() {
        let error = Lexer::new("1 + @ 2").tokenize().unwrap_err();
        // '@' is at line 1, column 5 (1-based).
        assert_eq!(error.position(), Some(SourcePosition { line: 1, column: 5 }));
        assert_eq!(error.to_string(), "unexpected character '@'");
    }

    #[test]
    fn rejects_standalone_exclamation() {
        assert_eq!(
            Lexer::new("1 ! 2").tokenize().unwrap_err().to_string(),
            "unexpected character '!'"
        );
    }

    #[test]
    fn rejects_out_of_range_literals() {
        assert_eq!(
            Lexer::new("18446744073709551616")
                .tokenize()
                .unwrap_err()
                .to_string(),
            "integer literal out of range"
        );
    }
}