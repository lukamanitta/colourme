mod config;

use std::env;
use std::fs;
use std::io::Write;
use std::process::exit;

use regex::Regex;
use toml::{Table, Value};

use colour_utils::Colour;
use config::Config;

extern crate shellexpand;

const TEMPLATE_EXPR_REGEX_STR: &str = r"\{\{(.*)\}\}";
const COLOUR_FORMAT_REGEX_STR: &str = r"\{\{(\w+):(.*)\}\}";
const FUNCTION_REGEX_STR: &str = r"(\w+)\((.*)\)";
const DOT_NOTATION_REGEX_STR: &str = r"\w+";

struct ColourDefinition {
    label: String,
    colour_str: String,
}

fn usage() {
    println!(
        "usage:
            colourme <string>"
    );
}

fn resolve_dot_notated_array_access(dot_notated_access: &str, table: &Table) -> Option<Value> {
    let mut intermediate_table = table.clone();
    let mut resolved_result: Option<Value> = None;
    let accesses = dot_notated_access.split(".");

    for access in accesses {
        match intermediate_table.get(access) {
            Some(entry) => match entry {
                Value::Table(val) => {
                    intermediate_table = val.clone();
                }
                val => resolved_result = Some(val.clone()),
            },
            None => {}
        }
    }
    return resolved_result;
}

fn main() {
    let argv: Vec<String> = env::args().collect();
    let colourscheme_name: &String;
    match argv.len() {
        2 => {
            colourscheme_name = &argv[1];
        }
        _ => {
            usage();
            exit(1);
        }
    };

    let colourme_dir = shellexpand::tilde("~/.config/colourme");
    let colourscheme_dir = format!("{}/schemes", colourme_dir);
    let colourscheme_path = format!("{}/{}.toml", colourscheme_dir, colourscheme_name);

    let colourscheme_content = match fs::read_to_string(&colourscheme_path) {
        Ok(c) => c,
        Err(why) => {
            eprintln!("Couldn't read file {colourscheme_path}: {why}");
            exit(1);
        }
    };
    let colourscheme_table = colourscheme_content.parse::<Table>().unwrap();

    let config_path = shellexpand::tilde("~/.config/colourme/config.toml").to_string();
    let config_content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(why) => {
            eprintln!("Couldn't read file {config_path}: {why}");
            exit(1);
        }
    };
    let config: Config = Config::new(&config_content);

    // This will be persistent across the various template files to avoid
    // re-calculating colours, as well as easily replacing them at the end
    let mut colour_definitions: Vec<ColourDefinition> = Vec::new();

    let template_expr_regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();
    let colour_format_regex = Regex::new(COLOUR_FORMAT_REGEX_STR).unwrap();
    let function_regex = Regex::new(FUNCTION_REGEX_STR).unwrap();
    let dot_notation_regex = Regex::new(DOT_NOTATION_REGEX_STR).unwrap();

    for entry in config.entries.iter() {
        let mut template_content = match fs::read_to_string(&entry.template_path) {
            Ok(c) => c,
            Err(why) => {
                eprintln!("Couldn't read file {}: {why}", &entry.template_path);
                exit(1);
            }
        };

        let template_expr_matches = template_expr_regex.find_iter(&template_content);
        for template_expr in template_expr_matches {
            // Bail if this expression has been encountered
            if colour_definitions
                .iter()
                .any(|def| def.label == template_expr.as_str())
            {
                continue;
            }

            let format_content_caps = colour_format_regex
                .captures(template_expr.as_str())
                .unwrap();

            let format = format_content_caps.get(1).map_or("", |c| c.as_str()); // TODO: throw error
            match format {
                "hex" | "rgb" | "hsv" => {}
                _ => {
                    continue;
                } // invalid colour format, exit
            };

            let content = format_content_caps.get(2).map_or("", |c| c.as_str()); // TODO: throw
                                                                                 // error

            // check if function regex matches
            // this is placeholder
            // let colour_function_identifier = regex capture
            let colour_function = |colour: Colour| -> Colour { colour };
            let colour_identifier = content.clone();

            // get result from dot notated access
            let resolved_colour_value: Option<Value> =
                resolve_dot_notated_array_access(colour_identifier, &colourscheme_table);
            match resolved_colour_value {
                Some(_) => {}
                None => {
                    continue;
                } // invalid colour identifier, exit
            };

            // check if it is a String
            let resolved_colour_str = match resolved_colour_value.unwrap() {
                Value::String(val) => Ok(val),
                _ => Err("Template entry is not a string"),
            };

            let resolved_colour_object =
                Colour::new(&resolved_colour_str.unwrap()).unwrap_or_else(|e| exit(1));

            let mutated_colour = colour_function(resolved_colour_object);

            let formatted_colour_str = match format {
                "hex" => mutated_colour.hex().to_string(),
                "rgb" => mutated_colour.rgb().to_string(),
                "hsv" => mutated_colour.hsv().to_string(),
                _ => {
                    continue;
                } // for some reason format conversion didn't happen, exit
            };

            colour_definitions.push(ColourDefinition {
                label: template_expr.as_str().to_string(),
                colour_str: formatted_colour_str,
            });
        }

        // Colour definitions are collected, now replace them in the temporary file contents
        for colour_definition in colour_definitions.iter() {
            template_content =
                template_content.replace(&colour_definition.label, &colour_definition.colour_str);
        }
        // Then replace escaped curly brackets with regular curly brackets
        template_content = template_content.replace(r"\{", r"{");

        let mut destination_file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&entry.destination_path)
            .unwrap();
        destination_file
            .write(&template_content.as_bytes())
            .unwrap();
        destination_file.flush().unwrap();
    }
}
