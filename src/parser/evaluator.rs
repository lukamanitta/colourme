use crate::parser::ast::{Expr, TemplateExpr};
use crate::parser::functions::{builtin_blend, builtin_darken, builtin_multiply_brightness};
use colour_utils::Colour;
use std::collections::HashMap;
use toml::{Table, Value as TomlValue};

// #[derive(Debug, Clone)]
pub enum Value {
    Colour(Colour),
    Number(f32),
}

pub type FunctionPtr = fn(&[Value]) -> Result<Value, String>;

pub struct Evaluator<'a> {
    toml_table: &'a Table,
    functions: HashMap<&'static str, FunctionPtr>,
}

impl<'a> Evaluator<'a> {
    pub fn new(toml_table: &'a Table) -> Self {
        let mut functions: HashMap<&'static str, FunctionPtr> = HashMap::new();
        functions.insert("darken", builtin_darken);
        functions.insert("blend", builtin_blend);
        functions.insert("multiply_brightness", builtin_multiply_brightness);

        Self {
            toml_table,
            functions,
        }
    }

    pub fn evaluate(&self, template: &TemplateExpr) -> Result<String, String> {
        let result = self.evaluate_expr(&template.expr)?;

        let final_colour = match result {
            Value::Colour(c) => c,
            Value::Number(_) => return Err("Expected a colour value, got a number".to_string()),
        };

        match template.format {
            "hex" => Ok(final_colour.hex().to_string()),
            "rgb" => Ok(final_colour.rgb().to_string()),
            "hsv" => Ok(final_colour.hsv().to_string()),
            _ => Err(format!("Unsupported format: {}", template.format)),
        }
    }

    pub fn evaluate_expr(&self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Number(n_str) => {
                let parsed = n_str
                    .parse::<f32>()
                    .map_err(|_| format!("Failed to parse number: {}", n_str))?;
                Ok(Value::Number(parsed))
            }

            Expr::Hex(hex_str) => {
                let colour = Colour::new(hex_str)
                    .map_err(|e| format!("Failed to parse hex colour '{}': {}", hex_str, e))?;
                Ok(Value::Colour(colour))
            }

            Expr::Identifier(path_parts) => {
                let mut current_value: Option<&TomlValue> = None;
                for part in path_parts {
                    current_value = match current_value {
                        None => self.toml_table.get(*part),
                        Some(TomlValue::Table(table)) => table.get(*part),
                        Some(_) => return Err(format!("Invalid path: '{}' is not a table", part)),
                    };
                }

                let final_str = match current_value {
                    Some(TomlValue::String(s)) => s,
                    Some(_) => {
                        return Err(format!(
                            "Expected a string at the end of the path, got {:?}",
                            current_value
                        ))
                    }
                    None => return Err(format!("Path not found in TOML: {:?}", path_parts)),
                };

                let colour = Colour::new(final_str).map_err(|e| {
                    format!(
                        "Failed to parse colour from TOML string '{}': {}",
                        final_str, e
                    )
                })?;
                Ok(Value::Colour(colour))
            }

            Expr::Function { name, args } => {
                let mut evaluated_args = Vec::new();
                for arg in args {
                    evaluated_args.push(self.evaluate_expr(arg)?);
                }

                let func = self
                    .functions
                    .get(name)
                    .ok_or_else(|| format!("Undefined function: {}", name))?;

                func(&evaluated_args)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Expr, TemplateExpr};
    use colour_utils::operations::darken;

    fn mock_toml() -> Table {
        let toml_str = r#"
            [colors]
            primary = '#FF0000'
            background = '#222222'

            [theme]
            accent = '#00FF00'
        "#;

        toml_str
            .parse::<Table>()
            .expect("Failed to parse mock TOML")
    }

    #[test]
    fn test_evaluate_literal_hex() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `hex:#AABBCC`
        let ast = TemplateExpr {
            format: "hex",
            expr: Expr::Hex("#AABBCC"),
        };

        let result = evaluator.evaluate(&ast).unwrap();

        // If your Colour struct automatically uppercases/lowercases, adjust this assertion!
        assert_eq!(result, "AABBCC");
    }

    #[test]
    fn test_evaluate_toml_identifier() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `hex:colors.primary`
        let ast = TemplateExpr {
            format: "hex",
            expr: Expr::Identifier(vec!["colors", "primary"]),
        };

        let result = evaluator.evaluate(&ast).unwrap();
        assert_eq!(result, "FF0000"); // Matches the TOML mock
    }

    #[test]
    fn test_formatting_conversion() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `rgb:colors.primary`
        let ast = TemplateExpr {
            format: "rgb",
            expr: Expr::Identifier(vec!["colors", "primary"]),
        };

        let result = evaluator.evaluate(&ast).unwrap();

        // Assuming your colour.rgb().to_string() outputs this format.
        // #FF0000 is Pure Red.
        assert_eq!(result, "rgb(255, 0, 0)");
    }

    #[test]
    fn test_evaluate_function() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `hex:darken(colors.primary, 0.5)`
        let ast = TemplateExpr {
            format: "hex",
            expr: Expr::Function {
                name: "darken",
                args: vec![
                    Expr::Identifier(vec!["colors", "primary"]), // Evaluates to Color
                    Expr::Number("0.5"),                         // Evaluates to Number
                ],
            },
        };

        let result = evaluator.evaluate(&ast).unwrap();

        // You will need to change "#800000" to whatever EXACT hex code
        // your `darken` function mathematically produces when darkening #FF0000 by 0.5.
        assert_eq!(
            result,
            darken(&Colour::new("#FF0000").unwrap(), 0.5)
                .unwrap()
                .hex()
                .to_string()
        );
    }

    // --- Error Handling Tests ---

    #[test]
    fn test_error_unknown_function() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `hex:magic_spell(#FFFFFF)`
        let ast = TemplateExpr {
            format: "hex",
            expr: Expr::Function {
                name: "magic_spell", // This doesn't exist in the HashMap
                args: vec![Expr::Hex("#FFFFFF")],
            },
        };

        let result = evaluator.evaluate(&ast);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Undefined function: magic_spell");
    }

    #[test]
    fn test_error_invalid_toml_path() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `hex:colors.does_not_exist`
        let ast = TemplateExpr {
            format: "hex",
            expr: Expr::Identifier(vec!["colors", "does_not_exist"]),
        };

        let result = evaluator.evaluate(&ast);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Path not found in TOML: [\"colors\", \"does_not_exist\"]"
        );
    }
}
