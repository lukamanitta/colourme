use crate::parser::evaluator::Value;
use colour_utils::operations::{blend, darken, multiply_brightness};

pub fn builtin_darken(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!(
            "darken function expects 2 arguments, got {}",
            args.len()
        ));
    }

    let colour = match &args[0] {
        Value::Colour(c) => c,
        _ => return Err("First argument to darken must be a colour".to_string()),
    };

    let amount = match &args[1] {
        Value::Number(n) => *n,
        _ => return Err("Second argument to darken must be a number".to_string()),
    };

    let darkened_colour = match darken(colour, amount) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to darken colour: {}", e)),
    };

    Ok(Value::Colour(darkened_colour))
}
