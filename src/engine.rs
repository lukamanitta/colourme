use crate::parser::{Evaluator, Lexer, Parser};
use toml::Table;

pub struct TemplateEngine<'a> {
    evaluator: Evaluator<'a>,
}

impl<'a> TemplateEngine<'a> {
    pub fn new(toml_table: &'a Table) -> Self {
        Self {
            evaluator: Evaluator::new(toml_table),
        }
    }

    pub fn resolve_block(&self, source: &str) -> Result<String, String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexer error in '{}': {}", source, e))?;

        let mut parser = Parser::new(tokens);
        let ast = parser
            .parse()
            .map_err(|e| format!("Parser error in '{}': {}", source, e))?;

        self.evaluator
            .evaluate(&ast)
            .map_err(|e| format!("Evaluation error in '{}': {}", source, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use colour_utils::operations::darken;
    use colour_utils::Colour;

    #[test]
    fn test_template_engine() {
        let toml_str = r#"
            [colors]
            primary = '#FF0000'
        "#;

        let toml_data = toml_str.parse::<Table>().expect("Failed to parse TOML");
        let engine = TemplateEngine::new(&toml_data);

        let result = engine
            .resolve_block("rgb:darken(colors.primary, 0.5)")
            .expect("Failed to resolve block");

        assert_eq!(
            result,
            darken(&Colour::new("#FF0000").unwrap(), 0.5)
                .unwrap()
                .rgb()
                .to_string()
        );
    }
}
