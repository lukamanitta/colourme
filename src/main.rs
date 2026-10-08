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

fn default_colourscheme_path(colourscheme_name: &str) -> String {
    format!(
        "{}/schemes/{}.toml",
        shellexpand::tilde("~/.config/colourme"),
        colourscheme_name
    )
}

fn default_config_path() -> String {
    shellexpand::tilde("~/.config/colourme/config.toml").to_string()
}

fn load_colorscheme_from_path(colourscheme_path: &str) -> Result<Table, String> {
    let content = fs::read_to_string(colourscheme_path)
        .map_err(|why| format!("Couldn't read file {}: {}", colourscheme_path, why))?;
    content
        .parse::<Table>()
        .map_err(|e| format!("Failed to parse {}: {}", colourscheme_path, e))
}

fn load_config_from_path(config_path: &str) -> Result<Config, String> {
    let content = fs::read_to_string(config_path)
        .map_err(|why| format!("Couldn't read file {}: {}", config_path, why))?;
    Config::new(&content)
}

fn collect_colour_definitions(
    template_content: &str,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    regex: &Regex,
) -> Result<(), String> {
    for template_expr in regex.find_iter(template_content) {
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

        let resolved_colour_str = engine.resolve_block(stripped_expr).map_err(|e| {
            format!(
                "Error resolving expression '{}': {}",
                template_expr.as_str(),
                e
            )
        })?;

        colour_definitions.push(ColourDefinition {
            label: template_expr.as_str().to_string(),
            colour_str: resolved_colour_str,
        });
    }

    Ok(())
}

fn replace_colour_definitions(
    template_content: &str,
    colour_definitions: &[ColourDefinition],
) -> String {
    let mut result = template_content.to_string();
    for colour_definition in colour_definitions {
        result = result.replace(&colour_definition.label, &colour_definition.colour_str);
    }
    result.replace(r"\{", r"{").replace(r"\}", r"}")
}

fn process_template_content(
    template_content: &str,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    regex: &Regex,
) -> Result<String, String> {
    collect_colour_definitions(template_content, engine, colour_definitions, regex)?;
    Ok(replace_colour_definitions(
        template_content,
        colour_definitions,
    ))
}

fn render_template_file(
    template_path: &str,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    regex: &Regex,
) -> Result<String, String> {
    let template_content = fs::read_to_string(template_path)
        .map_err(|why| format!("Couldn't read file {}: {}", template_path, why))?;
    process_template_content(&template_content, engine, colour_definitions, regex)
}

fn write_output(destination_path: &str, content: &str) -> Result<(), String> {
    let mut destination_file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(destination_path)
        .map_err(|e| format!("Failed to open {}: {}", destination_path, e))?;

    destination_file
        .write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write {}: {}", destination_path, e))?;
    destination_file
        .flush()
        .map_err(|e| format!("Failed to flush {}: {}", destination_path, e))
}

fn run_post_hook(
    entry_name: &str,
    post_hook: &str,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    regex: &Regex,
) -> Result<(), String> {
    let resolved_post_hook = resolve_post_hook(post_hook, engine, colour_definitions, regex)?;

    println!(
        "[{}] Executing post-hook: {}",
        entry_name, resolved_post_hook
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
                    entry_name, status
                );
            }
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "[{}] Failed to execute post-hook command '{}': {}",
                entry_name, resolved_post_hook, e
            );
            Ok(())
        }
    }
}

fn run_with_paths(colourscheme_path: &str, config_path: &str) -> Result<(), String> {
    let colourscheme_table = load_colorscheme_from_path(colourscheme_path)?;
    let config = load_config_from_path(config_path)?;

    // Persistent across templates to avoid re-calculating colours.
    let mut colour_definitions: Vec<ColourDefinition> = Vec::new();
    let engine = TemplateEngine::new(&colourscheme_table);
    let template_expr_regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

    for entry in config.entries.iter() {
        let resolved_content = render_template_file(
            &entry.template_path,
            &engine,
            &mut colour_definitions,
            &template_expr_regex,
        )?;

        println!(
            "[{}] Writing to {}...",
            &entry.name, &entry.destination_path
        );
        write_output(&entry.destination_path, &resolved_content)?;

        if let Some(post_hook) = &entry.post_hook {
            run_post_hook(
                &entry.name,
                post_hook,
                &engine,
                &mut colour_definitions,
                &template_expr_regex,
            )?;
        }
    }

    Ok(())
}

fn run(colourscheme_name: &str) -> Result<(), String> {
    let colourscheme_path = default_colourscheme_path(colourscheme_name);
    let config_path = default_config_path();
    run_with_paths(&colourscheme_path, &config_path)
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

    if let Err(e) = run(colourscheme_name) {
        eprintln!("{}", e);
        exit(1);
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

    fn write_temp_file(contents: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("colourme_test_{}_{}", std::process::id(), counter));
        std::fs::write(&path, contents).unwrap();
        path
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

    #[test]
    fn test_collect_colour_definitions_adds_and_skips_existing() {
        let toml_table = test_toml();
        let engine = TemplateEngine::new(&toml_table);
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();
        let mut definitions = vec![ColourDefinition {
            label: "{{hex:colors.primary}}".to_string(),
            colour_str: "FF0000".to_string(),
        }];

        let template = "{{hex:colors.primary}} and {{hex:colors.secondary}}";
        collect_colour_definitions(template, &engine, &mut definitions, &regex).unwrap();

        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[1].label, "{{hex:colors.secondary}}");
        assert_eq!(definitions[1].colour_str, "00FF00");
    }

    #[test]
    fn test_replace_colour_definitions_replaces_and_unescapes() {
        let definitions = vec![
            ColourDefinition {
                label: "{{hex:colors.primary}}".to_string(),
                colour_str: "FF0000".to_string(),
            },
            ColourDefinition {
                label: "{{hex:colors.secondary}}".to_string(),
                colour_str: "00FF00".to_string(),
            },
        ];
        let template = r"\{ {{hex:colors.primary}} and {{hex:colors.secondary}} \}";
        let result = replace_colour_definitions(template, &definitions);
        assert_eq!(result, "{ FF0000 and 00FF00 }");
    }

    #[test]
    fn test_process_template_content_resolves_and_updates_definitions() {
        let toml_table = test_toml();
        let engine = TemplateEngine::new(&toml_table);
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();
        let mut definitions = Vec::new();

        let template = r"\{ {{hex:colors.primary}} and {{hex:colors.secondary}} \}";
        let result = process_template_content(template, &engine, &mut definitions, &regex).unwrap();

        assert_eq!(result, "{ FF0000 and 00FF00 }");
        assert_eq!(definitions.len(), 2);
    }

    #[test]
    fn test_run_with_paths_renders_templates_and_writes_output() {
        let scheme_path = write_temp_file(
            r#"
            [colors]
            primary = '#FF0000'
            secondary = '#00FF00'
            "#,
        );

        let template_path = write_temp_file("{{hex:colors.primary}} and {{hex:colors.secondary}}");
        let dest_path = write_temp_file("");

        let config_content = format!(
            "[entry]\ntemplate = \"{}\"\ndestination = \"{}\"\npost_hook = \"true\"\n",
            template_path.display(),
            dest_path.display()
        );
        let config_path = write_temp_file(&config_content);

        run_with_paths(scheme_path.to_str().unwrap(), config_path.to_str().unwrap()).unwrap();

        let output = std::fs::read_to_string(&dest_path).unwrap();
        assert_eq!(output, "FF0000 and 00FF00");

        std::fs::remove_file(&scheme_path).ok();
        std::fs::remove_file(&template_path).ok();
        std::fs::remove_file(&config_path).ok();
        std::fs::remove_file(&dest_path).ok();
    }
}
