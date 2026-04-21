mod config;
mod engine;
mod parser;

use engine::TemplateEngine;

use std::env;
use std::fs;
use std::io::Write;
use std::process::exit;

use regex::Regex;
use toml::Table;

use config::Config;

extern crate shellexpand;

const TEMPLATE_EXPR_REGEX_STR: &str = r"\{\{(.*)\}\}";

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

    let engine = TemplateEngine::new(&colourscheme_table);

    let template_expr_regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

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

            let stripped_expr = template_expr
                .as_str()
                .trim_matches(|c| c == '{' || c == '}')
                .trim();

            let resolved_colour_str = match engine.resolve_block(stripped_expr) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "Error resolving expression '{}': {}",
                        template_expr.as_str(),
                        e
                    );
                    exit(1);
                }
            };

            colour_definitions.push(ColourDefinition {
                label: template_expr.as_str().to_string(),
                colour_str: resolved_colour_str,
            });
        }

        // Colour definitions are collected, now replace them in the temporary file contents
        for colour_definition in colour_definitions.iter() {
            template_content =
                template_content.replace(&colour_definition.label, &colour_definition.colour_str);
        }
        // Then replace escaped curly brackets with regular curly brackets
        template_content = template_content.replace(r"\{", r"{");

        println!(
            "[{}] Writing to {}...",
            &entry.name, &entry.destination_path
        );
        // println!("Resolved template content:\n{}", &template_content);

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
