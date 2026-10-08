use crate::parser::evaluator::Value;
use colour_utils::operations::{
    blend, darken, invert, invert_brightness, lighten, multiply_brightness,
};
use colour_utils::Colour;
use rand::seq::IndexedRandom;

fn colour_argument(
    args: &[Value],
    index: usize,
    ordinal: &str,
    function: &str,
) -> Result<Colour, String> {
    let invalid = || format!("{} argument to {} must be a colour", ordinal, function);
    match &args[index] {
        Value::Colour(c) => Ok(c.clone()),
        Value::String(s) => Colour::new(s).map_err(|_| invalid()),
        _ => Err(invalid()),
    }
}

pub fn builtin_h(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("h function expects 1 argument, got {}", args.len()));
    }

    let colour = colour_argument(args, 0, "First", "h")?;

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

    let colour = colour_argument(args, 0, "First", "darken")?;

    let amount = match &args[1] {
        Value::Number(n) => *n,
        _ => return Err("Second argument to darken must be a number".to_string()),
    };

    let darkened_colour =
        darken(&colour, amount).map_err(|e| format!("Failed to darken colour: {}", e))?;

    Ok(Value::Colour(darkened_colour))
}

pub fn builtin_lighten(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!(
            "lighten function expects 2 arguments, got {}",
            args.len()
        ));
    }

    let colour = colour_argument(args, 0, "First", "lighten")?;

    let multiplier = match &args[1] {
        Value::Number(n) => *n,
        _ => return Err("Second argument to lighten must be a number".to_string()),
    };

    let lightened_colour =
        lighten(&colour, multiplier).map_err(|e| format!("Failed to lighten colour: {}", e))?;

    Ok(Value::Colour(lightened_colour))
}

pub fn builtin_invert(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!(
            "invert function expects 1 argument, got {}",
            args.len()
        ));
    }

    let colour = colour_argument(args, 0, "First", "invert")?;

    let inverted_colour =
        invert(&colour).map_err(|e| format!("Failed to invert colour: {}", e))?;

    Ok(Value::Colour(inverted_colour))
}

pub fn builtin_invert_brightness(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!(
            "invert_brightness function expects 1 argument, got {}",
            args.len()
        ));
    }

    let colour = colour_argument(args, 0, "First", "invert_brightness")?;

    let inverted_colour =
        invert_brightness(&colour).map_err(|e| format!("Failed to invert brightness: {}", e))?;

    Ok(Value::Colour(inverted_colour))
}

pub fn builtin_blend(args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err(format!(
            "blend function expects 3 arguments, got {}",
            args.len()
        ));
    }

    let colour1 = colour_argument(args, 0, "First", "blend")?;
    let colour2 = colour_argument(args, 1, "Second", "blend")?;

    let ratio = match &args[2] {
        Value::Number(n) => *n,
        _ => return Err("Third argument to blend must be a number".to_string()),
    };

    let blended_colour =
        blend(&colour1, &colour2, ratio).map_err(|e| format!("Failed to blend colours: {}", e))?;

    Ok(Value::Colour(blended_colour))
}

pub fn builtin_multiply_brightness(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!(
            "multiply_brightness function expects 2 arguments, got {}",
            args.len()
        ));
    }

    let colour = colour_argument(args, 0, "First", "multiply_brightness")?;

    let multiplier = match &args[1] {
        Value::Number(n) => *n,
        _ => return Err("Second argument to multiply_brightness must be a number".to_string()),
    };

    let modified_colour = multiply_brightness(&colour, multiplier)
        .map_err(|e| format!("Failed to modify brightness: {}", e))?;

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

    #[test]
    fn test_builtin_h_reports_its_own_name_on_bad_string() {
        let err = builtin_h(&[Value::String("not-a-colour".to_string())]).unwrap_err();
        assert_eq!(err, "First argument to h must be a colour");
    }

    #[test]
    fn test_builtin_blend_uses_second_argument() {
        let args = vec![
            Value::Colour(Colour::new("#FF0000").unwrap()),
            Value::Colour(Colour::new("#0000FF").unwrap()),
            Value::Number(0.5),
        ];

        let result = builtin_blend(&args).unwrap();
        let expected = blend(
            &Colour::new("#FF0000").unwrap(),
            &Colour::new("#0000FF").unwrap(),
            0.5,
        )
        .unwrap();

        assert_eq!(result, Value::Colour(expected));
        // Blending with a distinct second colour must not reproduce the first.
        assert_ne!(result, args[0]);
    }

    #[test]
    fn test_builtin_blend_rejects_invalid_second_argument() {
        let args = vec![
            Value::Colour(Colour::new("#FF0000").unwrap()),
            Value::String("not-a-colour".to_string()),
            Value::Number(0.5),
        ];

        let err = builtin_blend(&args).unwrap_err();
        assert_eq!(err, "Second argument to blend must be a colour");
    }
}
