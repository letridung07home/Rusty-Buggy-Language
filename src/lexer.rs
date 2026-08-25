use crate::error::Error;

#[derive(Debug, PartialEq)]
pub(crate) enum Token {
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
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
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

            let token = match character {
                '=' => self.operator_with_optional_equals(Token::Equals, Token::EqualEqual),
                '<' => self.operator_with_optional_equals(Token::LessThan, Token::LessThanOrEqual),
                '>' => self
                    .operator_with_optional_equals(Token::GreaterThan, Token::GreaterThanOrEqual),
                '!' => {
                    self.advance();
                    if self.current_character() == Some('=') {
                        self.advance();
                        Token::NotEqual
                    } else {
                        return Err(Error::new("unexpected character '!'"));
                    }
                }
                ';' => {
                    self.advance();
                    Token::Semicolon
                }
                '+' => {
                    self.advance();
                    Token::Plus
                }
                '-' => {
                    self.advance();
                    Token::Minus
                }
                '*' => {
                    self.advance();
                    Token::Star
                }
                '/' => {
                    self.advance();
                    Token::Slash
                }
                '(' => {
                    self.advance();
                    Token::LeftParen
                }
                ')' => {
                    self.advance();
                    Token::RightParen
                }
                _ => return Err(Error::new(format!("unexpected character '{character}'"))),
            };
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn operator_with_optional_equals(&mut self, single: Token, compound: Token) -> Token {
        self.advance();
        if self.current_character() == Some('=') {
            self.advance();
            compound
        } else {
            single
        }
    }

    fn integer_literal(&mut self) -> Result<Token, Error> {
        let mut value = 0_u64;

        while let Some(character) = self.current_character() {
            if !character.is_ascii_digit() {
                break;
            }

            let digit = u64::from(character as u8 - b'0');
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(|| Error::new("integer literal out of range"))?;
            self.advance();
        }

        Ok(Token::Integer(value))
    }

    fn identifier(&mut self) -> Token {
        let start = self.position;

        while let Some(character) = self.current_character() {
            if !character.is_ascii_alphanumeric() && character != '_' {
                break;
            }
            self.advance();
        }

        let name = &self.input[start..self.position];
        if name == "let" {
            Token::Let
        } else {
            Token::Identifier(name.to_owned())
        }
    }

    fn current_character(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(character) = self.current_character() {
            self.position += character.len_utf8();
        }
    }
}

impl Token {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Integer(_) => "integer literal",
            Self::Identifier(_) => "identifier",
            Self::Let => "'let'",
            Self::Equals => "'='",
            Self::LessThan => "'<'",
            Self::LessThanOrEqual => "'<='",
            Self::GreaterThan => "'>'",
            Self::GreaterThanOrEqual => "'>='",
            Self::EqualEqual => "'=='",
            Self::NotEqual => "'!='",
            Self::Semicolon => "';'",
            Self::Plus => "'+'",
            Self::Minus => "'-'",
            Self::Star => "'*'",
            Self::Slash => "'/'",
            Self::LeftParen => "'('",
            Self::RightParen => "')'",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Lexer, Token};

    #[test]
    fn tokenizes_the_language_symbols_and_identifiers() {
        assert_eq!(
            Lexer::new("let _value2 = 1 <= 2 != 3;").tokenize(),
            Ok(vec![
                Token::Let,
                Token::Identifier("_value2".to_owned()),
                Token::Equals,
                Token::Integer(1),
                Token::LessThanOrEqual,
                Token::Integer(2),
                Token::NotEqual,
                Token::Integer(3),
                Token::Semicolon,
            ])
        );
    }

    #[test]
    fn rejects_invalid_characters() {
        assert_eq!(
            Lexer::new("1 @ 2").tokenize().unwrap_err().to_string(),
            "unexpected character '@'"
        );
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
