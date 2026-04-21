// Production rules:
// String ::= Letter | String Letter
// Toml ::= String | String "." Toml
// ColourFormat ::= "hex" | "rgb" | "rgba" | "hsv"
//
// HexColour ::= "#" HexSequence
// RGBColour ::= "rgb(" Num "," Num "," Num ")"
// RGBAColour ::= "rgba(" Num "," Num "," Num "," Num ")"
// HSVColour ::= "hsv(" Num "," Num "," Num ")"
//
// ColourIdentifier ::= Toml
// Colour ::= ColourIdentifier | HexColour | RGBColour | RGBAColour | HSVColour
//
// FunctionIdentifier ::= "darken" | "lighten" | "saturate" | "desaturate"
// FunctionArgs ::= Colour | Function | Colour "," FunctionArgs | Function "," FunctionArgs
// Function ::= FunctionIdentifier "(" FunctionArgs ")"
//
// HexSequence ::= HexDigit HexDigit HexDigit HexDigit HexDigit HexDigit
// HexDigit ::= "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "A" | "B" | "C" | "D" | "E" | "F" | "a" | "b" | "c" | "d" | "e" | "f"
// Num ::= Integer | Integer "." Integer
// Integer ::= Digit | Integer Digit
// Digit ::= "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
// Letter ::= "a" | "b" | "c" | "d" | "e" | "f" | "g" | "h" | "i" | "j" | "k" | "l" | "m" | "n" |
// "o" | "p" | "q" | "r" | "s" | "t" | "u" | "v" | "w" | "x" | "y" | "z" | "A" | "B" | "C" | "D" |
// "E" | "F" | "G" | "H" | "I" | "J" | "K" | "L" | "M" | "N" | "O" | "P" | "Q" | "R" | "S" | "T" |
// "U" | "V" | "W" | "X" | "Y" | "Z" | "_" | "-" | Digit

use super::ast::{Expr, TemplateExpr};
use super::token::Token;

pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    current: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<TemplateExpr<'a>, String> {
        // Consume the format
        let format = match self.advance() {
            Some(&Token::Word(w)) => w,
            _ => return Err("Expected format identifier (e.g. 'hex') at the start".to_string()),
        };

        // Consume the colon
        if !self.match_token(&Token::Colon) {
            return Err("Expected ':' after format identifier".to_string());
        }

        // Parse the expression
        let expr = self.parse_expression()?;

        // Ensure no trailing tokens
        if !self.is_at_end() {
            return Err(format!(
                "Unexpected token after expression: {:?}",
                self.peek()
            ));
        }

        Ok(TemplateExpr { format, expr })
    }

    fn parse_expression(&mut self) -> Result<Expr<'a>, String> {
        let token = self.advance().cloned();

        match token {
            Some(Token::Hex(h)) => Ok(Expr::Hex(h)),
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
            Some(Token::Word(first_word)) => {
                // Check if this is a function call
                if self.match_token(&Token::OpenParen) {
                    let args = self.parse_arguments()?;

                    if !self.match_token(&Token::CloseParen) {
                        return Err(format!("Expected ')' after arguments for '{}'", first_word));
                    }

                    Ok(Expr::Function {
                        name: first_word,
                        args,
                    })
                } else {
                    let mut path_parts = vec![first_word];

                    while self.match_token(&Token::Dot) {
                        match self.advance().cloned() {
                            Some(Token::Word(next_word)) => path_parts.push(next_word),
                            Some(t) => {
                                return Err(format!("Expected identifier after '.', found {:?}", t))
                            }
                            None => return Err("Unexpected end of input after '.'".to_string()),
                        }
                    }

                    Ok(Expr::Identifier(path_parts))
                }
            }

            Some(t) => Err(format!("Unexpected token in expression: {:?}", t)),
            None => Err("Unexpected end of input while parsing expression".to_string()),
        }
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr<'a>>, String> {
        let mut args = Vec::new();

        if self.check(&Token::CloseParen) {
            return Ok(args); // No arguments
        }

        loop {
            args.push(self.parse_expression()?);

            if self.match_token(&Token::Comma) {
                continue; // More arguments to parse
            } else {
                break;
            }
        }

        Ok(args)
    }

    fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.current)
    }
    fn advance(&mut self) -> Option<&Token<'a>> {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.tokens.get(self.current - 1)
    }
    fn match_token(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn check(&self, expected: &Token) -> bool {
        if self.is_at_end() {
            return false;
        }
        self.peek() == Some(expected)
    }
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::lexer::Lexer;

    fn parse_str(input: &str) -> TemplateExpr {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn test_parse_identifier() {
        let ast = parse_str("hex:colors.primary");
        assert_eq!(
            ast,
            TemplateExpr {
                format: "hex",
                expr: Expr::Identifier(vec!["colors", "primary"])
            }
        );
    }

    #[test]
    fn test_parse_function() {
        let ast = parse_str("rgb:darken(colors.bg, 0.5)");
        assert_eq!(
            ast,
            TemplateExpr {
                format: "rgb",
                expr: Expr::Function {
                    name: "darken",
                    args: vec![Expr::Identifier(vec!["colors", "bg"]), Expr::Number("0.5"),]
                }
            }
        );
    }

    #[test]
    fn test_parse_nested_functions() {
        let ast = parse_str("hsv:lighten(darken(colors.bg, 0.5), 0.2)");
        assert_eq!(
            ast,
            TemplateExpr {
                format: "hsv",
                expr: Expr::Function {
                    name: "lighten",
                    args: vec![
                        Expr::Function {
                            name: "darken",
                            args: vec![Expr::Identifier(vec!["colors", "bg"]), Expr::Number("0.5"),]
                        },
                        Expr::Number("0.2"),
                    ]
                }
            }
        );
    }

    #[test]
    fn test_missing_format_fails() {
        let mut lexer = Lexer::new("colors.primary"); // Missing "hex:"
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let result = parser.parse();

        assert!(result.is_err());
    }
}
