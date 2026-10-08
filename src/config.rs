use toml::Table;

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    pub name: String,
    pub template: String,
    pub destination: String,
    pub post_hook: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub entries: Vec<ConfigEntry>,
}

impl Config {
    pub fn new(toml_string: &str) -> Result<Config, String> {
        let config_table = toml_string
            .parse::<Table>()
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        let mut entries = Vec::new();
        for (key, value) in config_table.iter() {
            let entry_table = value
                .as_table()
                .ok_or_else(|| format!("Config entry '{}' must be a table", key))?;

            let template = entry_table
                .get("template")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    format!(
                        "Config entry '{}' is missing required string field 'template'",
                        key
                    )
                })?;

            let destination = entry_table
                .get("destination")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    format!(
                        "Config entry '{}' is missing required string field 'destination'",
                        key
                    )
                })?;

            let post_hook = entry_table
                .get("post_hook")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            entries.push(ConfigEntry {
                name: key.to_string(),
                template: template.to_string(),
                destination: destination.to_string(),
                post_hook,
            });
        }

        Ok(Config { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parses_entries() {
        let config = Config::new(
            r#"
            [hypr]
            template = "/t/hypr.lua"
            destination = "/d/hypr.lua"
            post_hook = "hyprctl reload"

            [ghostty]
            template = "/t/ghostty"
            destination = "/d/ghostty"
            "#,
        )
        .unwrap();

        assert_eq!(config.entries.len(), 2);
        let hypr = config.entries.iter().find(|e| e.name == "hypr").unwrap();
        assert_eq!(hypr.template, "/t/hypr.lua");
        assert_eq!(hypr.destination, "/d/hypr.lua");
        assert_eq!(hypr.post_hook.as_deref(), Some("hyprctl reload"));
        let ghostty = config.entries.iter().find(|e| e.name == "ghostty").unwrap();
        assert_eq!(ghostty.post_hook, None);
    }

    #[test]
    fn test_config_rejects_invalid_toml() {
        let err = Config::new("this is not toml =").unwrap_err();
        assert!(err.starts_with("Failed to parse config:"), "got: {}", err);
    }

    #[test]
    fn test_config_names_entry_missing_template() {
        let err = Config::new("[hypr]\ndestination = \"/d/hypr.lua\"\n").unwrap_err();
        assert!(err.contains("'hypr'"), "got: {}", err);
        assert!(err.contains("template"), "got: {}", err);
    }

    #[test]
    fn test_config_names_entry_missing_destination() {
        let err = Config::new("[hypr]\ntemplate = \"/t/hypr.lua\"\n").unwrap_err();
        assert!(err.contains("'hypr'"), "got: {}", err);
        assert!(err.contains("destination"), "got: {}", err);
    }

    #[test]
    fn test_config_rejects_non_string_template() {
        let err = Config::new("[hypr]\ntemplate = 3\ndestination = \"/d/hypr.lua\"\n").unwrap_err();
        assert!(err.contains("'hypr'"), "got: {}", err);
    }

    #[test]
    fn test_config_rejects_non_table_entry() {
        let err = Config::new("hypr = \"nope\"\n").unwrap_err();
        assert!(err.contains("'hypr'"), "got: {}", err);
    }
}
