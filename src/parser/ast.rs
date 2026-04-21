#[derive(Debug, PartialEq, Clone)]
pub struct TemplateExpr<'a> {
    pub format: &'a str,
    pub expr: Expr<'a>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr<'a> {
    // e.g. #ffffff
    Hex(&'a str),

    // e.g. 0.5 or 255
    Number(&'a str),

    // e.g. regular.red
    Identifier(Vec<&'a str>),

    // e.g. darken(...)
    Function { name: &'a str, args: Vec<Expr<'a>> },
}
