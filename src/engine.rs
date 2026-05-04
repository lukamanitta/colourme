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
        let block_ast = parser
            .parse()
            .map_err(|e| format!("Parser error in '{}': {}", source, e))?;

        let mut last_error = String::new();

        for fallback in block_ast.fallbacks {
            match self.evaluator.evaluate(&fallback) {
                Ok(result) => return Ok(result),
                Err(e) => last_error = e,
            }
        }

        Err(format!(
            "All fallbacks failed for '{}'. Last error: {}",
            source, last_error
        ))
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

    #[test]
    fn test_template_engine_takes_first_fallback() {
        let toml_str = r#"
            [colors]
            primary = '#FF0000'
        "#;

        let toml_data = toml_str.parse::<Table>().expect("Failed to parse TOML");
        let engine = TemplateEngine::new(&toml_data);

        let result = engine
            .resolve_block("hsv:colors.primary || hsv:colors.secondary")
            .expect("Failed to resolve block");

        assert_eq!(result, Colour::new("#FF0000").unwrap().hsv().to_string());
    }

    #[test]
    fn test_template_engine_takes_first_of_all_valid_fallbacks() {
        let toml_str = r#"
            [colors]
            primary = '#FF0000'
            secondary = '#00FF00'
        "#;

        let toml_data = toml_str.parse::<Table>().expect("Failed to parse TOML");
        let engine = TemplateEngine::new(&toml_data);

        let result = engine
            .resolve_block("hsv:colors.primary || hsv:colors.secondary")
            .expect("Failed to resolve block");

        assert_eq!(result, Colour::new("#FF0000").unwrap().hsv().to_string());
    }

    #[test]
    fn test_template_engine_takes_second_fallback() {
        let toml_str = r#"
            [colors]
            secondary = '#00FF00'
        "#;

        let toml_data = toml_str.parse::<Table>().expect("Failed to parse TOML");
        let engine = TemplateEngine::new(&toml_data);

        let result = engine
            .resolve_block("hsv:colors.primary || hsv:colors.secondary")
            .expect("Failed to resolve block");

        assert_eq!(result, Colour::new("#00FF00").unwrap().hsv().to_string());
    }

    #[test]
    fn test_template_engine_all_fallbacks_fail() {
        let toml_str = r#"
            [colors]
        "#;

        let toml_data = toml_str.parse::<Table>().expect("Failed to parse TOML");
        let engine = TemplateEngine::new(&toml_data);

        let result = engine.resolve_block("hsv:colors.primary || hsv:colors.secondary");

        assert!(result.is_err());
    }
}
