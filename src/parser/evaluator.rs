use crate::parser::ast::{Expr, TemplateExpr};
use crate::parser::functions::{builtin_blend, builtin_darken, builtin_multiply_brightness};
use colour_utils::Colour;
use std::collections::HashMap;
use toml::{Table, Value as TomlValue};

// #[derive(Debug, Clone)]
pub enum Value {
    Colour(Colour),
    Number(f32),
    String(String),
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

        match template.format {
            "hex" => match result {
                Value::String(s) => {
                    let colour = Colour::new(&s);
                    match colour {
                        Ok(c) => Ok(c.hex().to_string()),
                        Err(e) => Err(format!("Failed to parse '{}' as a colour: {}", s, e)),
                    }
                }
                Value::Colour(c) => Ok(c.hex().to_string()),
                Value::Number(_) => Err("Expected a hex value, got a number".to_string()),
            },
            "rgb" => match result {
                Value::String(s) => {
                    let colour = Colour::new(&s);
                    match colour {
                        Ok(c) => Ok(c.rgb().to_string()),
                        Err(e) => Err(format!("Failed to parse '{}' as a colour: {}", s, e)),
                    }
                }
                Value::Colour(c) => Ok(c.rgb().to_string()),
                Value::Number(_) => Err("Expected an rgb value, got a number".to_string()),
            },
            "hsv" => match result {
                Value::String(s) => {
                    let colour = Colour::new(&s);
                    match colour {
                        Ok(c) => Ok(c.hsv().to_string()),
                        Err(e) => Err(format!("Failed to parse '{}' as a colour: {}", s, e)),
                    }
                }
                Value::Colour(c) => Ok(c.hsv().to_string()),
                Value::Number(_) => Err("Expected an hsv value, got a number".to_string()),
            },
            "str" => match result {
                Value::String(s) => Ok(s),
                // Should only ever be hex if we get a Value::Colour at this point
                Value::Colour(c) => Ok(c.hex().to_string()),
                Value::Number(n) => Ok(n.to_string()),
            },
            "num" => match result {
                Value::Number(n) => Ok(n.to_string()),
                Value::String(s) => s
                    .parse::<f32>()
                    .map_err(|_| format!("Failed to parse '{}' as a number", s))
                    .map(|n| n.to_string()),
                Value::Colour(c) => Err(format!(
                    "Cannot convert colour '{}' to a number",
                    c.hex().to_string()
                )),
            },
            _ => Err(format!("Unsupported format: {}", template.format)),
        }
    }

    fn evaluate_expr(&self, expr: &Expr) -> Result<Value, String> {
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

                match current_value {
                    Some(&TomlValue::String(ref s)) => Ok(Value::String(s.clone())),
                    Some(&TomlValue::Integer(i)) => Ok(Value::Number(i as f32)),
                    Some(&TomlValue::Float(f)) => Ok(Value::Number(f as f32)),
                    Some(_) => {
                        return Err(format!(
                            "Expected a string at the end of the path, got {:?}",
                            current_value
                        ))
                    }
                    None => Err(format!("Path not found in TOML: {:?}", path_parts)),
                }
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
            some_string = "Some String"
            some_int = 42
            some_float = 3.14

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

    #[test]
    fn test_evaluate_string_format() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `str:some_string`
        let ast = TemplateExpr {
            format: "str",
            expr: Expr::Identifier(vec!["some_string"]),
        };

        let result = evaluator.evaluate(&ast).unwrap();
        assert_eq!(result, "Some String");
    }

    #[test]
    fn test_evaluate_number_format_int() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `num:some_int`
        let ast = TemplateExpr {
            format: "num",
            expr: Expr::Identifier(vec!["some_int"]),
        };

        let result = evaluator.evaluate(&ast).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_evaluate_number_format_float() {
        let toml_data = mock_toml();
        let evaluator = Evaluator::new(&toml_data);

        // AST for: `num:some_float`
        let ast = TemplateExpr {
            format: "num",
            expr: Expr::Identifier(vec!["some_float"]),
        };

        let result = evaluator.evaluate(&ast).unwrap();
        assert_eq!(result, "3.14");
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
