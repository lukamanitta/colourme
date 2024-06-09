use colour_utils::{Colour, darken};
use std::collections::HashMap;

// map function names to function pointers with variable arguments

// need to traverse down from first function, evaluating arguments
// arguments can be functions themselves, which need to be recursively evaluated

pub struct Parser {
    source: String,
    result: Result<Colour, String>,
    // need to abstract fn type
    defined_functions: HashMap<String, fn(Vec<Colour>, f64) -> Colour>,
}

impl Parser {
    pub fn new(source: String) -> Parser {
        Parser {
            source,
            result: Err("No result".to_string()),
            defined_functions: HashMap::from([
                ("darken".to_string(), darken),
            ]),
        }
    }

    pub fn parse(&mut self) {

    }

    pub fn result(&self) -> Result<Colour, String> {
        self.result
    }
}
