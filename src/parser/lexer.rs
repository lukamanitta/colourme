use std::iter::Peekable;
use std::str::CharIndices;

use super::token::Token;

pub struct Lexer<'a> {
    source: &'a str,
    chars: Peekable<CharIndices<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token<'a>>, String> {
        let mut tokens = Vec::new();

        while let Some(&(start, ch)) = self.chars.peek() {
            match ch {
                // Skip whitespace
                c if c.is_whitespace() => {
                    self.chars.next();
                }

                // Punctuation
                ':' => {
                    tokens.push(Token::Colon);
                    self.chars.next();
                }
                '.' => {
                    tokens.push(Token::Dot);
                    self.chars.next();
                }
                ',' => {
                    tokens.push(Token::Comma);
                    self.chars.next();
                }
                '(' => {
                    tokens.push(Token::OpenParen);
                    self.chars.next();
                }
                ')' => {
                    tokens.push(Token::CloseParen);
                    self.chars.next();
                }

                // Hex colours
                '#' => match self.get_hex_token() {
                    Ok(token) => tokens.push(token),
                    Err(e) => return Err(e),
                },

                // Numbers
                c if c.is_ascii_digit() => match self.get_number_token() {
                    Ok(token) => tokens.push(token),
                    Err(e) => return Err(e),
                },

                // Words (identifiers, function names, etc.)
                c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {
                    match self.get_word_token() {
                        Ok(token) => tokens.push(token),
                        Err(e) => return Err(e),
                    }
                }

                _ => {
                    return Err(format!(
                        "Unexpected character '{}' at position {}",
                        ch, start
                    ));
                }
            }
        }
        Ok(tokens)
    }

    fn get_hex_token(&mut self) -> Result<Token<'a>, String> {
        let start = self.chars.peek().unwrap().0;
        let mut end = start;

        // Consume the '#' character
        self.chars.next();
        end += 1;

        while let Some(&(idx, ch)) = self.chars.peek() {
            if ch.is_ascii_hexdigit() {
                end = idx + ch.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let hex_str = &self.source[start..end];
        let len = hex_str.len() - 1; // Exclude the '#' character

        if len != 6 {
            return Err(format!("Invalid hex colour: {}", hex_str));
        }

        Ok(Token::Hex(hex_str))
    }

    fn get_number_token(&mut self) -> Result<Token<'a>, String> {
        let start = self.chars.peek().unwrap().0;
        let mut end = start;

        while let Some(&(idx, ch)) = self.chars.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                end = idx + ch.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let num_str = &self.source[start..end];
        Ok(Token::Number(num_str))
    }

    fn get_word_token(&mut self) -> Result<Token<'a>, String> {
        let start = self.chars.peek().unwrap().0;
        let mut end = start;

        while let Some(&(idx, ch)) = self.chars.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                end = idx + ch.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let word_str = &self.source[start..end];
        Ok(Token::Word(word_str))
    }
}

// Test cases

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_no_words() {
        let input = "rgb:darken(rgb(255, 0, 0), 0.5)";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("rgb"),
                Token::Colon,
                Token::Word("darken"),
                Token::OpenParen,
                Token::Word("rgb"),
                Token::OpenParen,
                Token::Number("255"),
                Token::Comma,
                Token::Number("0"),
                Token::Comma,
                Token::Number("0"),
                Token::CloseParen,
                Token::Comma,
                Token::Number("0.5"),
                Token::CloseParen,
            ]
        );
    }

    #[test]
    fn test_lexer_with_words() {
        let input = "hex:darken(regular.red, 0.5)";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("hex"),
                Token::Colon,
                Token::Word("darken"),
                Token::OpenParen,
                Token::Word("regular"),
                Token::Dot,
                Token::Word("red"),
                Token::Comma,
                Token::Number("0.5"),
                Token::CloseParen,
            ]
        );
    }

    #[test]
    fn test_lexer_hex_colour() {
        let input = "rgb:darken(#ff0000, 0.5)";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("rgb"),
                Token::Colon,
                Token::Word("darken"),
                Token::OpenParen,
                Token::Hex("#ff0000"),
                Token::Comma,
                Token::Number("0.5"),
                Token::CloseParen,
            ]
        );
    }

    #[test]
    fn test_lexer_invalid_hex() {
        let input = "rgb:darken(#ff000, 0.5)";
        let mut lexer = Lexer::new(input);
        let result = lexer.tokenize();
        assert!(result.is_err());
    }
}
