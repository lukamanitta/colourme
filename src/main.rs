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

use clap::Parser;
use regex::Regex;
use toml::Table;

use config::{Config, ConfigEntry};

const TEMPLATE_EXPR_REGEX_STR: &str = r"\{\{(.*?)\}\}";

/// Render a colour scheme into the files declared in a config.
#[derive(Parser, Debug)]
#[command(
    name = "colourme",
    version,
    about = "Render colour scheme templates into config files",
    disable_help_subcommand = true
)]
struct Cli {
    /// Scheme name to render, or "list" to list available schemes.
    #[arg(value_name = "SCHEME")]
    scheme: String,

    /// Config TOML to use.
    #[arg(long, value_name = "PATH", env = "COLOURME_CONFIG")]
    config: Option<PathBuf>,

    /// Directory of `<scheme>.toml` files.
    #[arg(long = "schemes-dir", value_name = "DIR", env = "COLOURME_SCHEMES_DIR")]
    schemes_dir: Option<PathBuf>,

    /// Re-root `$HOME`-based destinations under this directory.
    #[arg(long = "dest-root", value_name = "DIR")]
    dest_root: Option<PathBuf>,

    /// Render and write, but run no post-hook commands.
    #[arg(long = "no-hooks")]
    no_hooks: bool,

    /// Resolve and render everything, but run nothing and write nothing.
    #[arg(long = "dry-run")]
    dry_run: bool,
}

struct ColourDefinition {
    label: String,
    colour_str: String,
}

/// Everything needed to render a scheme, with all input paths resolved.
struct RunOptions {
    config_path: PathBuf,
    schemes_dir: PathBuf,
    dest_root: Option<PathBuf>,
    home: PathBuf,
    cwd: PathBuf,
    no_hooks: bool,
    dry_run: bool,
}

impl RunOptions {
    fn from_cli(cli: &Cli) -> Result<Self, String> {
        let home = paths::home_dir()?;
        let cwd = paths::current_dir()?;
        let xdg_env = env::var("XDG_CONFIG_HOME").ok();
        let xdg = paths::xdg_config_home(xdg_env.as_deref(), &home);

        Ok(Self {
            config_path: paths::resolve_config_path(cli.config.as_deref(), &home, &xdg),
            schemes_dir: paths::resolve_schemes_dir(cli.schemes_dir.as_deref(), &home, &xdg),
            dest_root: cli
                .dest_root
                .as_deref()
                .map(|root| paths::resolve_dest_root(root, &cwd)),
            home,
            cwd,
            no_hooks: cli.no_hooks,
            dry_run: cli.dry_run,
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

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&resolved_post_hook)
        .status()
        .map_err(|e| {
            format!(
                "[{}] Failed to execute post-hook command '{}': {}",
                entry_name, resolved_post_hook, e
            )
        })?;

    if !status.success() {
        return Err(format!(
            "[{}] Post-hook command exited with non-zero status {}: {}",
            entry_name, status, resolved_post_hook
        ));
    }

    Ok(())
}

/// Warn about scheme keys referenced by a template that are entirely absent,
/// skipping any block that uses `||` fallbacks (where the author has already
/// opted into optional keys). This surfaces a genuinely missing key before the
/// render-time error and without changing fallback behaviour. See REVIEW O8.
fn missing_scheme_keys(
    engine: &TemplateEngine,
    config: &Config,
    options: &RunOptions,
    regex: &Regex,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut warned = std::collections::HashSet::new();

    for entry in &config.entries {
        let template_path =
            paths::resolve_template(&entry.template, options.config_dir(), &options.home);
        let Ok(content) = fs::read_to_string(&template_path) else {
            continue;
        };

        for captures in regex.captures_iter(&content) {
            let source = captures[1].trim();
            let Some(references) = engine.references(source) else {
                continue;
            };
            if references.has_fallback {
                continue;
            }

            for path in references.identifiers {
                if engine.path_exists(&path) {
                    continue;
                }
                let key = path.join(".");
                if warned.insert((entry.name.clone(), key.clone())) {
                    warnings.push(format!(
                        "[{}] scheme has no key '{}' referenced by {}",
                        entry.name,
                        key,
                        template_path.display()
                    ));
                }
            }
        }
    }

    warnings
}

/// Render every entry in `config` using an already-loaded scheme file.
fn run_scheme_file(options: &RunOptions, colourscheme_path: &Path) -> Result<(), String> {
    let colourscheme_table = load_colorscheme_from_path(colourscheme_path)?;
    let config = load_config_from_path(&options.config_path)?;

    // Colour definitions are cached by the exact literal template text (e.g.
    // "{{hex:colors.primary}}") for the whole run, across every entry. This is
    // what makes random_select repeatable: the same expression reused in
    // several templates draws once and then reuses that value. Changing the
    // whitespace or argument order of an expression changes its key and thus
    // produces an independent draw (REVIEW O9).
    let mut colour_definitions: Vec<ColourDefinition> = Vec::new();
    let engine = TemplateEngine::new(&colourscheme_table);
    let template_expr_regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

    for warning in missing_scheme_keys(&engine, &config, options, &template_expr_regex) {
        eprintln!("warning: {}", warning);
    }

    let mut hook_failures: Vec<String> = Vec::new();
    for entry in config.entries.iter() {
        render_entry(
            entry,
            &engine,
            &mut colour_definitions,
            &template_expr_regex,
            options,
            &mut hook_failures,
        )?;
    }

    if !hook_failures.is_empty() {
        return Err(format!(
            "{} post-hook(s) failed:\n{}",
            hook_failures.len(),
            hook_failures.join("\n")
        ));
    }

    Ok(())
}

fn render_entry(
    entry: &ConfigEntry,
    engine: &TemplateEngine,
    colour_definitions: &mut Vec<ColourDefinition>,
    regex: &Regex,
    options: &RunOptions,
    hook_failures: &mut Vec<String>,
) -> Result<(), String> {
    let template_path =
        paths::resolve_template(&entry.template, options.config_dir(), &options.home);
    let destination = paths::resolve_destination(
        &entry.destination,
        options.dest_root.as_deref(),
        &options.home,
        &options.cwd,
    );

    let resolved_content =
        render_template_file(&template_path, engine, colour_definitions, regex)?;

    if options.dry_run {
        println!(
            "[{}] would write {} ({} bytes)",
            entry.name,
            destination.display(),
            resolved_content.len()
        );
        if let Some(post_hook) = &entry.post_hook {
            let resolved = resolve_post_hook(post_hook, engine, colour_definitions, regex)?;
            println!("[{}] would run post-hook: {}", entry.name, resolved);
        }
        return Ok(());
    }

    println!("[{}] Writing to {}...", entry.name, destination.display());
    output::write_atomically(&destination, &resolved_content)?;

    if let Some(post_hook) = &entry.post_hook {
        if options.no_hooks {
            println!("[{}] skipping post-hook (--no-hooks)", entry.name);
        } else if let Err(e) = run_post_hook(&entry.name, post_hook, engine, colour_definitions, regex)
        {
            hook_failures.push(e);
        }
    }

    Ok(())
}

fn available_schemes(schemes_dir: &Path) -> Result<Vec<String>, String> {
    let read_dir = fs::read_dir(schemes_dir).map_err(|e| {
        format!(
            "Couldn't read schemes directory {}: {}",
            schemes_dir.display(),
            e
        )
    })?;

    let mut names = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|e| {
            format!(
                "Couldn't read schemes directory {}: {}",
                schemes_dir.display(),
                e
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with('_') {
            continue;
        }
        names.push(stem.to_string());
    }

    names.sort();
    Ok(names)
}

fn list_schemes(schemes_dir: &Path) -> Result<(), String> {
    for name in available_schemes(schemes_dir)? {
        println!("{}", name);
    }
    Ok(())
}

fn run_with_options(options: &RunOptions, colourscheme_name: &str) -> Result<(), String> {
    let colourscheme_path = options
        .schemes_dir
        .join(format!("{}.toml", colourscheme_name));
    run_scheme_file(options, &colourscheme_path)
}

fn execute(cli: &Cli) -> Result<(), String> {
    let options = RunOptions::from_cli(cli)?;

    if cli.scheme == "list" {
        list_schemes(&options.schemes_dir)
    } else {
        run_with_options(&options, &cli.scheme)
    }
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = execute(&cli) {
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
            dest_root: None,
            home: paths::home_dir().unwrap(),
            cwd: paths::current_dir().unwrap(),
            no_hooks: false,
            dry_run: false,
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "colourme_{}_{}_{}",
            prefix,
            std::process::id(),
            counter
        ));
        fs::create_dir_all(&path).unwrap();
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

    #[test]
    fn test_cli_parses_flags_and_scheme() {
        let cli = Cli::try_parse_from([
            "colourme",
            "--config",
            "/tmp/c.toml",
            "--schemes-dir",
            "/tmp/s",
            "--dest-root",
            "/out",
            "--dry-run",
            "Gruvbox",
        ])
        .unwrap();

        assert_eq!(cli.scheme, "Gruvbox");
        assert_eq!(cli.config.as_deref(), Some(Path::new("/tmp/c.toml")));
        assert_eq!(cli.schemes_dir.as_deref(), Some(Path::new("/tmp/s")));
        assert_eq!(cli.dest_root.as_deref(), Some(Path::new("/out")));
        assert!(cli.dry_run);
    }

    #[test]
    fn test_cli_requires_a_scheme() {
        assert!(Cli::try_parse_from(["colourme"]).is_err());
    }

    #[test]
    fn test_cli_help_and_version_are_display_errors() {
        use clap::error::ErrorKind;
        let help = Cli::try_parse_from(["colourme", "--help"]).unwrap_err();
        assert_eq!(help.kind(), ErrorKind::DisplayHelp);
        let version = Cli::try_parse_from(["colourme", "--version"]).unwrap_err();
        assert_eq!(version.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_from_cli_uses_explicit_paths() {
        let cli = Cli::try_parse_from([
            "colourme",
            "--config",
            "/tmp/c.toml",
            "--schemes-dir",
            "/tmp/s",
            "X",
        ])
        .unwrap();

        let options = RunOptions::from_cli(&cli).unwrap();
        assert_eq!(options.config_path, PathBuf::from("/tmp/c.toml"));
        assert_eq!(options.schemes_dir, PathBuf::from("/tmp/s"));
    }

    #[test]
    fn test_available_schemes_sorted_and_filtered() {
        let dir = unique_temp_dir("list");
        fs::write(dir.join("Gruvbox.toml"), "").unwrap();
        fs::write(dir.join("Catppuccin.toml"), "").unwrap();
        fs::write(dir.join("_template.toml"), "").unwrap();
        fs::write(dir.join("notes.txt"), "").unwrap();

        let names = available_schemes(&dir).unwrap();
        assert_eq!(names, vec!["Catppuccin", "Gruvbox"]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_available_schemes_errors_on_missing_dir() {
        let missing = unique_temp_dir("missing").join("nope");
        assert!(available_schemes(&missing).is_err());
    }

    #[test]
    fn test_dest_root_re_roots_home_destinations() {
        let dir = unique_temp_dir("destroot");
        let home = dir.join("home");
        let out = dir.join("out");
        fs::create_dir_all(&home).unwrap();

        let scheme_path = dir.join("scheme.toml");
        fs::write(&scheme_path, "[colors]\nprimary = '#FF0000'\n").unwrap();

        let template_path = dir.join("template.txt");
        fs::write(&template_path, "{{hex:colors.primary}}").unwrap();

        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            format!(
                "[entry]\ntemplate = \"{}\"\ndestination = \"~/.config/x/out.txt\"\n",
                template_path.display()
            ),
        )
        .unwrap();

        let options = RunOptions {
            config_path,
            schemes_dir: PathBuf::new(),
            dest_root: Some(out.clone()),
            home,
            cwd: dir.clone(),
            no_hooks: false,
            dry_run: false,
        };
        run_scheme_file(&options, &scheme_path).unwrap();

        assert_eq!(
            fs::read_to_string(out.join(".config/x/out.txt")).unwrap(),
            "FF0000"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_dry_run_writes_nothing_and_runs_no_hook() {
        let dir = unique_temp_dir("dryrun");
        let sentinel = dir.join("sentinel");

        let scheme_path = dir.join("scheme.toml");
        fs::write(&scheme_path, "[colors]\nprimary = '#FF0000'\n").unwrap();

        let template_path = dir.join("template.txt");
        fs::write(&template_path, "{{hex:colors.primary}}").unwrap();

        let dest_path = dir.join("out.txt");
        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            format!(
                "[entry]\ntemplate = \"{}\"\ndestination = \"{}\"\npost_hook = \"touch {}\"\n",
                template_path.display(),
                dest_path.display(),
                sentinel.display()
            ),
        )
        .unwrap();

        let mut options = test_options(&config_path);
        options.dry_run = true;
        run_scheme_file(&options, &scheme_path).unwrap();

        assert!(!dest_path.exists(), "dry-run must not write output");
        assert!(!sentinel.exists(), "dry-run must not run post-hooks");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_no_hooks_writes_output_but_skips_hook() {
        let dir = unique_temp_dir("nohooks");
        let sentinel = dir.join("sentinel");

        let scheme_path = dir.join("scheme.toml");
        fs::write(&scheme_path, "[colors]\nprimary = '#FF0000'\n").unwrap();

        let template_path = dir.join("template.txt");
        fs::write(&template_path, "{{hex:colors.primary}}").unwrap();

        let dest_path = dir.join("out.txt");
        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            format!(
                "[entry]\ntemplate = \"{}\"\ndestination = \"{}\"\npost_hook = \"touch {}\"\n",
                template_path.display(),
                dest_path.display(),
                sentinel.display()
            ),
        )
        .unwrap();

        let mut options = test_options(&config_path);
        options.no_hooks = true;
        run_scheme_file(&options, &scheme_path).unwrap();

        assert_eq!(fs::read_to_string(&dest_path).unwrap(), "FF0000");
        assert!(!sentinel.exists(), "--no-hooks must not run post-hooks");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_hooks_run_by_default() {
        let dir = unique_temp_dir("hooks");
        let sentinel = dir.join("sentinel");

        let scheme_path = dir.join("scheme.toml");
        fs::write(&scheme_path, "[colors]\nprimary = '#FF0000'\n").unwrap();

        let template_path = dir.join("template.txt");
        fs::write(&template_path, "{{hex:colors.primary}}").unwrap();

        let dest_path = dir.join("out.txt");
        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            format!(
                "[entry]\ntemplate = \"{}\"\ndestination = \"{}\"\npost_hook = \"touch {}\"\n",
                template_path.display(),
                dest_path.display(),
                sentinel.display()
            ),
        )
        .unwrap();

        let options = test_options(&config_path);
        run_scheme_file(&options, &scheme_path).unwrap();

        assert!(sentinel.exists(), "post-hook should run without --no-hooks");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_run_post_hook_reports_failure() {
        let table = test_toml();
        let engine = TemplateEngine::new(&table);
        let mut definitions = Vec::new();
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

        let err = run_post_hook("entry", "exit 3", &engine, &mut definitions, &regex).unwrap_err();
        assert!(err.contains("non-zero"), "got: {}", err);
    }

    #[test]
    fn test_run_scheme_file_continues_after_hook_failure() {
        let dir = unique_temp_dir("hookfail");

        let scheme_path = dir.join("scheme.toml");
        fs::write(&scheme_path, "[colors]\nprimary = '#FF0000'\n").unwrap();

        let template_path = dir.join("template.txt");
        fs::write(&template_path, "{{hex:colors.primary}}").unwrap();

        let dest_a = dir.join("a.txt");
        let dest_b = dir.join("b.txt");
        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            format!(
                "[alpha]\ntemplate = \"{t}\"\ndestination = \"{a}\"\npost_hook = \"exit 1\"\n\n\
                 [beta]\ntemplate = \"{t}\"\ndestination = \"{b}\"\n",
                t = template_path.display(),
                a = dest_a.display(),
                b = dest_b.display(),
            ),
        )
        .unwrap();

        let options = test_options(&config_path);
        let result = run_scheme_file(&options, &scheme_path);

        assert!(result.is_err(), "a failed post-hook must surface as an error");
        assert!(dest_a.exists(), "entry before the failure still written");
        assert!(dest_b.exists(), "entry after the failure still written");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_missing_scheme_keys_warns_without_fallbacks_only() {
        let dir = unique_temp_dir("schemecheck");
        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            format!(
                "[single]\ntemplate = \"{single}\"\ndestination = \"/tmp/x\"\n\n\
                 [fallback]\ntemplate = \"{fallback}\"\ndestination = \"/tmp/y\"\n",
                single = dir.join("single.txt").display(),
                fallback = dir.join("fallback.txt").display(),
            ),
        )
        .unwrap();
        fs::write(dir.join("single.txt"), "{{hex:colors.accent}}").unwrap();
        fs::write(
            dir.join("fallback.txt"),
            "{{hex:colors.accent || hex:colors.primary}}",
        )
        .unwrap();

        let scheme = "[colors]\nprimary = '#FF0000'\n".parse::<Table>().unwrap();
        let engine = TemplateEngine::new(&scheme);
        let config = load_config_from_path(&config_path).unwrap();
        let options = test_options(&config_path);
        let regex = Regex::new(TEMPLATE_EXPR_REGEX_STR).unwrap();

        let warnings = missing_scheme_keys(&engine, &config, &options, &regex);

        assert_eq!(warnings.len(), 1, "got: {:?}", warnings);
        assert!(warnings[0].contains("colors.accent"), "got: {}", warnings[0]);
        assert!(warnings[0].contains("[single]"), "got: {}", warnings[0]);

        fs::remove_dir_all(&dir).ok();
    }
}
