use crate::parser::ast::Expr;
use crate::parser::{Evaluator, Lexer, Parser};
use toml::Table;

/// The scheme keys referenced by a single `{{ ... }}` template block.
pub struct BlockReferences {
    /// True when the block contains `||` fallbacks, meaning the author has
    /// explicitly allowed some referenced keys to be absent.
    pub has_fallback: bool,
    pub identifiers: Vec<Vec<String>>,
}

fn collect_identifiers(expr: &Expr, out: &mut Vec<Vec<String>>) {
    match expr {
        Expr::Identifier(parts) => out.push(parts.iter().map(|s| s.to_string()).collect()),
        Expr::Function { args, .. } => {
            for arg in args {
                collect_identifiers(arg, out);
            }
        }
        Expr::Hex(_) | Expr::Number(_) => {}
    }
}

pub struct TemplateEngine<'a> {
    evaluator: Evaluator<'a>,
}

impl<'a> TemplateEngine<'a> {
    pub fn new(toml_table: &'a Table) -> Self {
        Self {
            evaluator: Evaluator::new(toml_table),
        }
    }

    /// Whether `path` resolves to a value in the scheme table.
    pub fn path_exists(&self, path: &[String]) -> bool {
        self.evaluator.path_exists(path)
    }

    /// The scheme keys referenced by `source`, or `None` if it does not parse.
    /// Used to warn about missing keys before rendering.
    pub fn references(&self, source: &str) -> Option<BlockReferences> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().ok()?;
        let mut parser = Parser::new(tokens);
        let block = parser.parse().ok()?;

        let mut identifiers = Vec::new();
        for fallback in &block.fallbacks {
            collect_identifiers(&fallback.expr, &mut identifiers);
        }

        Some(BlockReferences {
            has_fallback: block.fallbacks.len() > 1,
            identifiers,
        })
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
