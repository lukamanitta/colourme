#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Word(&'a str),
    Number(&'a str),
    Hex(&'a str),
    Colon,
    Dot,
    Comma,
    OpenParen,
    CloseParen,
}
