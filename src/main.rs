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

const TEMPLATE_EXPR_REGEX_STR: &str = r"\{\{(.*?)\}\}";

struct ColourDefinition {
    label: String,
    colour_str: String,
}

fn resolve_post_hook(
    post_hook: &str,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    template_expr_regex: &Regex,
) -> Result<String, String> {
    let mut resolved_post_hook = post_hook.to_string();

    let post_hook_expr_matches: Vec<String> = template_expr_regex
        .find_iter(post_hook)
        .map(|m| m.as_str().to_string())
        .collect();

    for template_expr in post_hook_expr_matches {
        if let Some(existing) = colour_definitions
            .iter()
            .find(|def| def.label == template_expr)
        {
            resolved_post_hook = resolved_post_hook.replace(&existing.label, &existing.colour_str);
            continue;
        }

        let stripped_expr = template_expr.trim_matches(|c| c == '{' || c == '}').trim();

        let resolved_colour_str = engine.resolve_block(stripped_expr).map_err(|e| {
            format!(
                "Error resolving post-hook expression '{}': {}",
                template_expr, e
            )
        })?;

        colour_definitions.push(ColourDefinition {
            label: template_expr.clone(),
            colour_str: resolved_colour_str.clone(),
        });

        resolved_post_hook = resolved_post_hook.replace(&template_expr, &resolved_colour_str);
    }

    Ok(resolved_post_hook)
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

        if let Some(post_hook) = &entry.post_hook {
            let resolved_post_hook = match resolve_post_hook(
                post_hook,
                &engine,
                &mut colour_definitions,
                &template_expr_regex,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(1);
                }
            };

            println!(
                "[{}] Executing post-hook: {}",
                &entry.name, resolved_post_hook
            );
            match std::process::Command::new("sh")
                .arg("-c")
                .arg(&resolved_post_hook)
                .status()
            {
                Ok(status) => {
                    if !status.success() {
                        eprintln!(
                            "[{}] Post-hook command exited with non-zero status: {}",
                            &entry.name, status
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[{}] Failed to execute post-hook command '{}': {}",
                        &entry.name, resolved_post_hook, e
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_toml() -> Table {
        let toml_str = r#"
            [colors]
            primary = '#FF0000'
            secondary = '#00FF00'
        "#;
        toml_str.parse::<Table>().unwrap()
    }

    #[test]
    fn test_post_hook_resolves_single_expression() {
        let toml_table = test_toml();
        let engine = TemplateEngine::new(&toml_table);
        let mut colour_definitions = Vec::new();
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

        let post_hook = "swaymsg output * bg {{hex:colors.primary}}";
        let resolved =
            resolve_post_hook(post_hook, &engine, &mut colour_definitions, &regex).unwrap();

        assert_eq!(resolved, "swaymsg output * bg FF0000");
        assert_eq!(colour_definitions.len(), 1);
        assert_eq!(colour_definitions[0].label, "{{hex:colors.primary}}");
        assert_eq!(colour_definitions[0].colour_str, "FF0000");
    }

    #[test]
    fn test_post_hook_reuses_existing_definition() {
        let toml_table = test_toml();
        let engine = TemplateEngine::new(&toml_table);
        let mut colour_definitions = vec![ColourDefinition {
            label: "{{hex:colors.primary}}".to_string(),
            colour_str: "FF0000".to_string(),
        }];
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

        let post_hook = "notify-send {{hex:colors.primary}}";
        let resolved =
            resolve_post_hook(post_hook, &engine, &mut colour_definitions, &regex).unwrap();

        assert_eq!(resolved, "notify-send FF0000");
        assert_eq!(colour_definitions.len(), 1); // No new definition added
    }

    #[test]
    fn test_post_hook_resolves_multiple_and_new() {
        let toml_table = test_toml();
        let engine = TemplateEngine::new(&toml_table);
        let mut colour_definitions = vec![ColourDefinition {
            label: "{{hex:colors.primary}}".to_string(),
            colour_str: "FF0000".to_string(),
        }];
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

        let post_hook = "{{hex:colors.primary}} and {{hex:colors.secondary}}";
        let resolved =
            resolve_post_hook(post_hook, &engine, &mut colour_definitions, &regex).unwrap();

        assert_eq!(resolved, "FF0000 and 00FF00");
        assert_eq!(colour_definitions.len(), 2);
        assert_eq!(colour_definitions[1].label, "{{hex:colors.secondary}}");
        assert_eq!(colour_definitions[1].colour_str, "00FF00");
    }

    #[test]
    fn test_post_hook_error_on_invalid_expression() {
        let toml_table = test_toml();
        let engine = TemplateEngine::new(&toml_table);
        let mut colour_definitions = Vec::new();
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

        let post_hook = "cmd {{hex:colors.missing}}";
        let result = resolve_post_hook(post_hook, &engine, &mut colour_definitions, &regex);

        assert!(result.is_err());
    }
}
