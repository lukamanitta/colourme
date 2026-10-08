mod config;
mod engine;
mod output;
mod parser;
mod paths;

use engine::TemplateEngine;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use regex::Regex;
use toml::Table;

use config::{Config, ConfigEntry};

const TEMPLATE_EXPR_REGEX_STR: &str = r"\{\{(.*?)\}\}";

struct ColourDefinition {
    label: String,
    colour_str: String,
}

/// Everything needed to render a scheme, with all input paths resolved.
struct RunOptions {
    config_path: PathBuf,
    schemes_dir: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
}

impl RunOptions {
    /// Build options from the XDG defaults (no CLI overrides yet).
    fn defaults() -> Result<Self, String> {
        let home = paths::home_dir()?;
        let cwd = paths::current_dir()?;
        let xdg_env = env::var("XDG_CONFIG_HOME").ok();
        let xdg = paths::xdg_config_home(xdg_env.as_deref(), &home);

        Ok(Self {
            config_path: paths::resolve_config_path(None, &home, &xdg),
            schemes_dir: paths::resolve_schemes_dir(None, &home, &xdg),
            home,
            cwd,
        })
    }

    fn config_dir(&self) -> &Path {
        self.config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
    }
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

fn load_colorscheme_from_path(colourscheme_path: &Path) -> Result<Table, String> {
    let content = fs::read_to_string(colourscheme_path)
        .map_err(|why| format!("Couldn't read file {}: {}", colourscheme_path.display(), why))?;
    content
        .parse::<Table>()
        .map_err(|e| format!("Failed to parse {}: {}", colourscheme_path.display(), e))
}

fn load_config_from_path(config_path: &Path) -> Result<Config, String> {
    let content = fs::read_to_string(config_path)
        .map_err(|why| format!("Couldn't read file {}: {}", config_path.display(), why))?;
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
    template_path: &Path,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    regex: &Regex,
) -> Result<String, String> {
    let template_content = fs::read_to_string(template_path)
        .map_err(|why| format!("Couldn't read file {}: {}", template_path.display(), why))?;
    process_template_content(&template_content, engine, colour_definitions, regex)
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

/// Render every entry in `config` using an already-loaded scheme file.
fn run_scheme_file(options: &RunOptions, colourscheme_path: &Path) -> Result<(), String> {
    let colourscheme_table = load_colorscheme_from_path(colourscheme_path)?;
    let config = load_config_from_path(&options.config_path)?;

    // Persistent across templates to avoid re-calculating colours.
    let mut colour_definitions: Vec<ColourDefinition> = Vec::new();
    let engine = TemplateEngine::new(&colourscheme_table);
    let template_expr_regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

    for entry in config.entries.iter() {
        render_entry(
            entry,
            &engine,
            &mut colour_definitions,
            &template_expr_regex,
            options,
        )?;
    }

    Ok(())
}

fn render_entry(
    entry: &ConfigEntry,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    regex: &Regex,
    options: &RunOptions,
) -> Result<(), String> {
    let template_path =
        paths::resolve_template(&entry.template, options.config_dir(), &options.home);
    let destination = paths::resolve_destination(&entry.destination, None, &options.home, &options.cwd);

    let resolved_content =
        render_template_file(&template_path, engine, colour_definitions, regex)?;

    println!("[{}] Writing to {}...", entry.name, destination.display());
    output::write_atomically(&destination, &resolved_content)?;

    if let Some(post_hook) = &entry.post_hook {
        run_post_hook(
            &entry.name,
            post_hook,
            engine,
            colour_definitions,
            regex,
        )?;
    }

    Ok(())
}

fn run(colourscheme_name: &str) -> Result<(), String> {
    let options = RunOptions::defaults()?;
    let colourscheme_path = options
        .schemes_dir
        .join(format!("{}.toml", colourscheme_name));
    run_scheme_file(&options, &colourscheme_path)
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

    fn write_temp_file(contents: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("colourme_test_{}_{}", std::process::id(), counter));
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn test_options(config_path: &Path) -> RunOptions {
        RunOptions {
            config_path: config_path.to_path_buf(),
            schemes_dir: PathBuf::new(),
            home: paths::home_dir().unwrap(),
            cwd: paths::current_dir().unwrap(),
        }
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
    fn test_run_scheme_file_renders_templates_and_writes_output() {
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

        let options = test_options(&config_path);
        run_scheme_file(&options, &scheme_path).unwrap();

        let output = std::fs::read_to_string(&dest_path).unwrap();
        assert_eq!(output, "FF0000 and 00FF00");

        std::fs::remove_file(&scheme_path).ok();
        std::fs::remove_file(&template_path).ok();
        std::fs::remove_file(&config_path).ok();
        std::fs::remove_file(&dest_path).ok();
    }

    #[test]
    fn test_relative_template_resolves_against_config_dir() {
        let config_dir = std::env::temp_dir().join(format!(
            "colourme_rel_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let templates_dir = config_dir.join("t");
        fs::create_dir_all(&templates_dir).unwrap();

        let scheme_path = config_dir.join("scheme.toml");
        fs::write(&scheme_path, "[colors]\nprimary = '#FF0000'\n").unwrap();

        fs::write(templates_dir.join("out.txt"), "{{hex:colors.primary}}").unwrap();

        let dest_path = config_dir.join("dest.txt");
        let config_path = config_dir.join("config.toml");
        fs::write(
            &config_path,
            format!(
                "[entry]\ntemplate = \"t/out.txt\"\ndestination = \"{}\"\n",
                dest_path.display()
            ),
        )
        .unwrap();

        let options = test_options(&config_path);
        run_scheme_file(&options, &scheme_path).unwrap();

        assert_eq!(fs::read_to_string(&dest_path).unwrap(), "FF0000");

        fs::remove_dir_all(&config_dir).ok();
    }
}
