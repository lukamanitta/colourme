use crate::parser::evaluator::Value;
use colour_utils::operations::{blend, darken, multiply_brightness};
use colour_utils::Colour;
use rand::seq::IndexedRandom;

pub fn builtin_h(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("h function expects 1 argument, got {}", args.len()));
    }

    let mut constructed_colour: Option<Colour> = None;
    let colour = match &args[0] {
        Value::Colour(ref c) => c,
        Value::String(s) => {
            constructed_colour = match Colour::new(s) {
                Ok(c) => Some(c),
                Err(_) => return Err("First argument to darken must be a colour".to_string()),
            };
            constructed_colour.as_ref().unwrap()
        }
        _ => return Err("Argument to h must be a colour".to_string()),
    };

    Ok(Value::Number(*colour.hsv().h()))
}

pub fn builtin_hsv(args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err(format!(
            "hsv function expects 3 arguments, got {}",
            args.len()
        ));
    }

    let h = match &args[0] {
        Value::Number(n) => *n,
        _ => return Err("First argument to hsv must be a number".to_string()),
    };

    let s = match &args[1] {
        Value::Number(n) => *n,
        _ => return Err("Second argument to hsv must be a number".to_string()),
    };

    let v = match &args[2] {
        Value::Number(n) => *n,
        _ => return Err("Third argument to hsv must be a number".to_string()),
    };

    let colour = Colour::new_from_hsv(h, s, v)
        .map_err(|e| format!("Failed to create colour from HSV: {}", e))?;
    Ok(Value::Colour(colour))
}

pub fn builtin_darken(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!(
            "darken function expects 2 arguments, got {}",
            args.len()
        ));
    }

    let mut constructed_colour: Option<Colour> = None;
    let colour = match &args[0] {
        Value::Colour(ref c) => c,
        Value::String(s) => {
            constructed_colour = match Colour::new(s) {
                Ok(c) => Some(c),
                Err(_) => return Err("First argument to darken must be a colour".to_string()),
            };
            constructed_colour.as_ref().unwrap()
        }
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

    let mut constructed_colour1: Option<Colour> = None;
    let colour1 = match &args[0] {
        Value::Colour(ref c) => c,
        Value::String(s) => {
            constructed_colour1 = match Colour::new(s) {
                Ok(c) => Some(c),
                Err(_) => return Err("First argument to blend must be a colour".to_string()),
            };
            constructed_colour1.as_ref().unwrap()
        }
        _ => return Err("First argument to blend must be a colour".to_string()),
    };

    let mut constructed_colour2: Option<Colour>;
    let colour2 = match &args[0] {
        Value::Colour(ref c) => c,
        Value::String(s) => {
            constructed_colour2 = match Colour::new(s) {
                Ok(c) => Some(c),
                Err(_) => return Err("Second argument to blend must be a colour".to_string()),
            };
            constructed_colour2.as_ref().unwrap()
        }
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

    let mut constructed_colour: Option<Colour> = None;
    let colour = match &args[0] {
        Value::Colour(ref c) => c,
        Value::String(s) => {
            constructed_colour = match Colour::new(s) {
                Ok(c) => Some(c),
                Err(_) => {
                    return Err("First argument to multiply_brightness must be a colour".to_string())
                }
            };
            constructed_colour.as_ref().unwrap()
        }
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

pub fn builtin_random_select(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("random_select function expects at least 1 argument".to_string());
    }

    let mut rng = rand::rng();
    if let Some(selected) = args.choose(&mut rng) {
        Ok(selected.clone())
    } else {
        Err("Failed to select a random value".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::evaluator::Value;

    #[test]
    fn test_builtin_random_select() {
        let args = vec![
            Value::String("apple".to_string()),
            Value::String("banana".to_string()),
            Value::String("cherry".to_string()),
        ];

        let result = builtin_random_select(&args);
        assert!(result.is_ok());
        let selected_value = result.unwrap();
        assert!(args.contains(&selected_value));
    }
}
