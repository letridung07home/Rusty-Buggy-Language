use crate::error::{Error, SourcePosition};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) position: SourcePosition,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Integer(u64),
    String(String),
    Identifier(String),
    Let,
    Fn,
    True,
    False,
    If,
    Else,
    Comma,
    Equals,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    EqualEqual,
    NotEqual,
    Bang,
    AndAnd,
    OrOr,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
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

            if character == '/' && self.comment_starts_here() {
                self.skip_comment()?;
                continue;
            }

            if character == '"' {
                tokens.push(self.string_literal()?);
                continue;
            }

            let position = self.current_position();
            let kind = match character {
                '=' => self.operator_with_optional_equals(TokenKind::Equals, TokenKind::EqualEqual),
                '<' => self
                    .operator_with_optional_equals(TokenKind::LessThan, TokenKind::LessThanOrEqual),
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
                        TokenKind::Bang
                    }
                }
                '&' => {
                    self.advance();
                    if self.current_character() == Some('&') {
                        self.advance();
                        TokenKind::AndAnd
                    } else {
                        return Err(Error::at("unexpected character '&'", position));
                    }
                }
                '|' => {
                    self.advance();
                    if self.current_character() == Some('|') {
                        self.advance();
                        TokenKind::OrOr
                    } else {
                        return Err(Error::at("unexpected character '|'", position));
                    }
                }
                ';' => {
                    self.advance();
                    TokenKind::Semicolon
                }
                ',' => {
                    self.advance();
                    TokenKind::Comma
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
                '%' => {
                    self.advance();
                    TokenKind::Percent
                }
                '(' => {
                    self.advance();
                    TokenKind::LeftParen
                }
                ')' => {
                    self.advance();
                    TokenKind::RightParen
                }
                '{' => {
                    self.advance();
                    TokenKind::LeftBrace
                }
                '}' => {
                    self.advance();
                    TokenKind::RightBrace
                }
                _ => {
                    return Err(self.error_at_current(format!("unexpected character '{character}'")))
                }
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

    fn operator_with_optional_equals(
        &mut self,
        single: TokenKind,
        compound: TokenKind,
    ) -> TokenKind {
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
        let kind = match name {
            "let" => TokenKind::Let,
            "fn" => TokenKind::Fn,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            _ => TokenKind::Identifier(name.to_owned()),
        };

        Token { kind, position }
    }

    /// Lexes a `"..."` string literal starting at the current `"` character
    /// and returns its decoded content. The supported escapes are `\n`, `\t`,
    /// `\\`, and `\"`; any other escape sequence is an error, and a literal
    /// that runs past the end of the input or contains a raw newline is
    /// reported as unterminated at the opening quote.
    fn string_literal(&mut self) -> Result<Token, Error> {
        let position = self.current_position();
        self.advance(); // consume the opening quote
        let mut value = String::new();

        loop {
            match self.current_character() {
                None => return Err(Error::at("unterminated string literal", position)),
                Some('"') => {
                    self.advance();
                    return Ok(Token {
                        kind: TokenKind::String(value),
                        position,
                    });
                }
                Some('\\') => {
                    self.advance(); // consume the backslash
                    let escape_position = self.current_position();
                    match self.current_character() {
                        Some('n') => {
                            value.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            value.push('\t');
                            self.advance();
                        }
                        Some('\\') => {
                            value.push('\\');
                            self.advance();
                        }
                        Some('"') => {
                            value.push('"');
                            self.advance();
                        }
                        _ => {
                            return Err(Error::at(
                                "invalid escape sequence in string literal",
                                escape_position,
                            ))
                        }
                    }
                }
                Some('\n') => return Err(Error::at("unterminated string literal", position)),
                Some(character) => {
                    value.push(character);
                    self.advance();
                }
            }
        }
    }

    /// Whether the current '/' begins a `//` line comment or `/*` block
    /// comment rather than the division operator.
    fn comment_starts_here(&self) -> bool {
        matches!(self.peek_character(), Some('/') | Some('*'))
    }

    /// The character immediately after the current one, if any.
    fn peek_character(&self) -> Option<char> {
        self.input[self.position..].chars().nth(1)
    }

    /// Skips a `//` line comment or `/* ... */` block comment that starts at
    /// the current '/' character. Block comments do not nest; an unterminated
    /// block comment is an error positioned at its opening `/*`.
    fn skip_comment(&mut self) -> Result<(), Error> {
        let position = self.current_position();
        self.advance(); // consume the '/'

        match self.current_character() {
            Some('/') => {
                self.advance(); // consume the second '/'
                while let Some(character) = self.current_character() {
                    if character == '\n' {
                        break;
                    }
                    self.advance();
                }
            }
            Some('*') => {
                self.advance(); // consume the '*'
                while let Some(character) = self.current_character() {
                    if character == '*' {
                        self.advance();
                        if self.current_character() == Some('/') {
                            self.advance();
                            return Ok(());
                        }
                    } else {
                        self.advance();
                    }
                }
                return Err(Error::at("unterminated block comment", position));
            }
            _ => unreachable!("skip_comment requires '/' followed by '/' or '*'"),
        }

        Ok(())
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
            TokenKind::String(_) => "string literal",
            TokenKind::Identifier(_) => "identifier",
            TokenKind::Let => "'let'",
            TokenKind::Fn => "'fn'",
            TokenKind::True => "'true'",
            TokenKind::False => "'false'",
            TokenKind::If => "'if'",
            TokenKind::Else => "'else'",
            TokenKind::Comma => "','",
            TokenKind::Equals => "'='",
            TokenKind::LessThan => "'<'",
            TokenKind::LessThanOrEqual => "'<='",
            TokenKind::GreaterThan => "'>'",
            TokenKind::GreaterThanOrEqual => "'>='",
            TokenKind::EqualEqual => "'=='",
            TokenKind::NotEqual => "'!='",
            TokenKind::Bang => "'!'",
            TokenKind::AndAnd => "'&&'",
            TokenKind::OrOr => "'||'",
            TokenKind::Semicolon => "';'",
            TokenKind::Plus => "'+'",
            TokenKind::Minus => "'-'",
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::Percent => "'%'",
            TokenKind::LeftParen => "'('",
            TokenKind::RightParen => "')'",
            TokenKind::LeftBrace => "'{'",
            TokenKind::RightBrace => "'}'",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use crate::error::SourcePosition;
    use crate::lexer::TokenKind;

    fn kinds(input: &str) -> Vec<TokenKind> {
        Lexer::new(input)
            .tokenize()
            .unwrap()
            .iter()
            .map(|token| token.kind.clone())
            .collect()
    }

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
    fn tokenizes_the_v2_keywords() {
        assert_eq!(
            kinds("let if else true false"),
            vec![
                TokenKind::Let,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::True,
                TokenKind::False,
            ]
        );
    }

    #[test]
    fn keywords_are_not_identifiers() {
        // Longer names that merely start with a keyword stay identifiers.
        assert_eq!(
            kinds("letter iffy elsewise truthful"),
            vec![
                TokenKind::Identifier("letter".to_owned()),
                TokenKind::Identifier("iffy".to_owned()),
                TokenKind::Identifier("elsewise".to_owned()),
                TokenKind::Identifier("truthful".to_owned()),
            ]
        );
    }

    #[test]
    fn tokenizes_bang_and_logical_operators() {
        assert_eq!(
            kinds("!true && false || true"),
            vec![
                TokenKind::Bang,
                TokenKind::True,
                TokenKind::AndAnd,
                TokenKind::False,
                TokenKind::OrOr,
                TokenKind::True,
            ]
        );
    }

    #[test]
    fn tokenizes_braces() {
        assert_eq!(
            kinds("{ }"),
            vec![TokenKind::LeftBrace, TokenKind::RightBrace]
        );
    }

    #[test]
    fn rejects_lone_ampersand_and_pipe() {
        let error = Lexer::new("1 & 2").tokenize().unwrap_err();
        assert_eq!(error.to_string(), "unexpected character '&'");
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 1, column: 3 })
        );

        let error = Lexer::new("1 | 2").tokenize().unwrap_err();
        assert_eq!(error.to_string(), "unexpected character '|'");
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 1, column: 3 })
        );
    }

    #[test]
    fn tokenizes_string_literals_with_escapes() {
        assert_eq!(
            kinds(r#""hello""#),
            vec![TokenKind::String("hello".to_owned())]
        );
        assert_eq!(kinds(r#""""#), vec![TokenKind::String(String::new())]);
        assert_eq!(
            kinds(r#""a\nb\tc\\d\"e""#),
            vec![TokenKind::String("a\nb\tc\\d\"e".to_owned())]
        );
    }

    #[test]
    fn string_literals_keep_their_opening_position() {
        let tokens = Lexer::new("let s = \"hi\"; s").tokenize().unwrap();

        assert_eq!(tokens[3].position, SourcePosition { line: 1, column: 9 });
    }

    #[test]
    fn rejects_unterminated_string_literals_at_the_opening_quote() {
        let error = Lexer::new("1 + \"oops").tokenize().unwrap_err();
        assert_eq!(error.to_string(), "unterminated string literal");
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 1, column: 5 })
        );
    }

    #[test]
    fn rejects_raw_newlines_inside_string_literals() {
        let error = Lexer::new("\"first\nsecond\"").tokenize().unwrap_err();
        assert_eq!(error.to_string(), "unterminated string literal");
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 1, column: 1 })
        );
    }

    #[test]
    fn rejects_invalid_escape_sequences_at_the_escape_character() {
        let error = Lexer::new(r#""a\x""#).tokenize().unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid escape sequence in string literal"
        );
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 1, column: 4 })
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
        assert_eq!(tokens[5].position, SourcePosition { line: 2, column: 3 });
        // '+' at line 2, column 5
        assert_eq!(tokens[6].position, SourcePosition { line: 2, column: 5 });
        // '2' at line 2, column 7
        assert_eq!(tokens[7].position, SourcePosition { line: 2, column: 7 });
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
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 1, column: 5 })
        );
        assert_eq!(error.to_string(), "unexpected character '@'");
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

    #[test]
    fn skips_line_comments_without_emitting_tokens() {
        let tokens = Lexer::new("1 + 2 // trailing comment").tokenize().unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|token| token.kind.clone()).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Integer(1),
                TokenKind::Plus,
                TokenKind::Integer(2)
            ]
        );
    }

    #[test]
    fn line_comments_end_at_the_newline_and_positions_stay_correct() {
        let tokens = Lexer::new("1 + // note\n  2").tokenize().unwrap();

        // The '2' after the comment sits at line 2, column 3.
        assert_eq!(tokens[2].position, SourcePosition { line: 2, column: 3 });
    }

    #[test]
    fn line_comments_run_to_the_end_of_the_input() {
        let tokens = Lexer::new("1 + 2 // no trailing newline")
            .tokenize()
            .unwrap();

        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn skips_block_comments_including_empty_and_multiline_ones() {
        for input in [
            "1 /* comment */ + 2",
            "1/**/+2",
            "1 /*\nmulti\nline\n*/ + 2",
        ] {
            let tokens = Lexer::new(input).tokenize().unwrap();
            let kinds: Vec<TokenKind> = tokens.iter().map(|token| token.kind.clone()).collect();

            assert_eq!(
                kinds,
                vec![
                    TokenKind::Integer(1),
                    TokenKind::Plus,
                    TokenKind::Integer(2)
                ],
                "input: {input}"
            );
        }
    }

    #[test]
    fn block_comment_newlines_advance_line_and_column_tracking() {
        let tokens = Lexer::new("1 /* first\nsecond */ + 2").tokenize().unwrap();

        // The '+' after the block comment is at line 2, column 11.
        assert_eq!(
            tokens[1].position,
            SourcePosition {
                line: 2,
                column: 11
            }
        );
    }

    #[test]
    fn block_comments_do_not_nest() {
        let tokens = Lexer::new("/* /* */ 1").tokenize().unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Integer(1));
    }

    #[test]
    fn rejects_unterminated_block_comments_with_the_opening_position() {
        let error = Lexer::new("1 + /* oops").tokenize().unwrap_err();

        assert_eq!(error.to_string(), "unterminated block comment");
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 1, column: 5 })
        );
    }

    #[test]
    fn rejects_unterminated_block_comments_on_later_lines() {
        let error = Lexer::new("1 + 2\n/* never closed").tokenize().unwrap_err();

        assert_eq!(error.to_string(), "unterminated block comment");
        assert_eq!(
            error.position(),
            Some(SourcePosition { line: 2, column: 1 })
        );
    }

    #[test]
    fn tokenizes_the_percent_operator() {
        let tokens = Lexer::new("1 % 2").tokenize().unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|token| token.kind.clone()).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Integer(1),
                TokenKind::Percent,
                TokenKind::Integer(2)
            ]
        );
    }

    #[test]
    fn a_single_slash_remains_the_division_operator() {
        let tokens = Lexer::new("8 / 2").tokenize().unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|token| token.kind.clone()).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Integer(8),
                TokenKind::Slash,
                TokenKind::Integer(2)
            ]
        );
    }

    #[test]
    fn slash_followed_by_space_and_star_is_not_a_comment() {
        let tokens = Lexer::new("1 / * 2").tokenize().unwrap();
        let kinds: Vec<TokenKind> = tokens.iter().map(|token| token.kind.clone()).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Integer(1),
                TokenKind::Slash,
                TokenKind::Star,
                TokenKind::Integer(2),
            ]
        );
    }
}
