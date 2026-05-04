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

pub fn builtin_blend(args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err(format!(
            "blend function expects 3 arguments, got {}",
            args.len()
        ));
    }

    let colour1 = match &args[0] {
        Value::Colour(c) => c,
        _ => return Err("First argument to blend must be a colour".to_string()),
    };

    let colour2 = match &args[1] {
        Value::Colour(c) => c,
        _ => return Err("Second argument to blend must be a colour".to_string()),
    };

    let ratio = match &args[2] {
        Value::Number(n) => *n,
        _ => return Err("Third argument to blend must be a number".to_string()),
    };

    let blended_colour = match blend(colour1, colour2, ratio) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to blend colours: {}", e)),
    };

    Ok(Value::Colour(blended_colour))
}

pub fn builtin_multiply_brightness(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!(
            "multiply_brightness function expects 2 arguments, got {}",
            args.len()
        ));
    }

    let colour = match &args[0] {
        Value::Colour(c) => c,
        _ => return Err("First argument to multiply_brightness must be a colour".to_string()),
    };

    let multiplier = match &args[1] {
        Value::Number(n) => *n,
        _ => return Err("Second argument to multiply_brightness must be a number".to_string()),
    };

    let modified_colour = match multiply_brightness(colour, multiplier) {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to modify brightness: {}", e)),
    };

    Ok(Value::Colour(modified_colour))
}
