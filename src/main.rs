mod config;

use std::env;
use std::fs;
use std::io::Write;
use std::process::exit;

use colour_utils::errors::InvalidColourFormat;
use toml::{Table, Value};
use regex::Regex;

use colour_utils::Colour;
use config::Config;

extern crate shellexpand;

const COLOUR_EXPR_REGEX_STR: &str = r"\{\{\w+:(?:\w|.)+\}\}";
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

fn main() {
    let argv: Vec<String> = env::args().collect();
    let colourscheme_name: &String;
    match argv.len() {
        2 => {
            colourscheme_name = &argv[1];
        }
        _ => {
            usage();
            return;
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

    let mut colour_definitions: Vec<ColourDefinition> = Vec::new();

    let colour_expr_regex = Regex::new(COLOUR_EXPR_REGEX_STR).unwrap();
    let dot_notation_regex = Regex::new(DOT_NOTATION_REGEX_STR).unwrap();

    for entry in config.entries.iter() {
        println!("Processing {} template...", entry.name);
        let mut template_content = match fs::read_to_string(&entry.template_path) {
            Ok(c) => c,
            Err(why) => {
                eprintln!("Couldn't read file {}: {why}", &entry.template_path);
                exit(1);
            }
        };
        let colour_expr_matches = colour_expr_regex.find_iter(&template_content);

        // Add new colour definitions
        for colour_expr in colour_expr_matches {

            // check if already exists in colour definitions

            let mut internal_matches = dot_notation_regex.find_iter(&colour_expr.as_str());
            let format = match internal_matches.next() {
                Some(format) => format.as_str(),
                None => { continue }, // something wrong with placeholder
            };

            let mut computed_colour_entry = colourscheme_table.clone();
            let mut colour_result: Option<Result<Colour, InvalidColourFormat>> = None;
            for dot_access in internal_matches {
                let colour_entry = match computed_colour_entry.get(dot_access.as_str()) {
                    Some(entry) => entry,
                    None => { break }, // tried to access non-existent entry
                };
                match colour_entry {
                    Value::String(val) => {
                        colour_result = Some(Colour::new(val));
                    }
                    Value::Table(val) => {
                        computed_colour_entry = val.clone();
                    }
                    _ => { break }
                }
            }
            // format colour and add to colour definitions
            let colour = match colour_result {
                Some(colour_result) => {
                    colour_result.unwrap()
                }
                None => {
                    continue;
                }
            };

            let colour_str = match format {
                "hex" => colour.hex().to_string(),
                "rgb" => colour.rgb().to_string(),
                "hsv" => colour.hsv().to_string(),
                _ => { continue; },
            };

            colour_definitions.push(
                ColourDefinition {
                    label: colour_expr.as_str().to_string(),
                    colour_str,
                }
            );
        }
        // Colour definitions are collected, now replace them in the temporary file contents
        for colour_definition in colour_definitions.iter() {
            template_content = template_content.replace(&colour_definition.label, &colour_definition.colour_str);
        }
        // Then replace escaped curly brackets with regular curly brackets
        template_content = template_content.replace(r"\{", r"{");

        let mut destination_file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&entry.destination_path)
            .unwrap();
        destination_file.write(&template_content.as_bytes()).unwrap();
        destination_file.flush().unwrap();
    }
}