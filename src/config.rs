use serde::Deserialize;
use toml::Table;

extern crate shellexpand;

#[derive(Debug, Deserialize)]
pub struct ConfigEntry {
    pub name: String,
    pub template_path: String,
    pub destination_path: String,
    pub post_hook: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub entries: Vec<ConfigEntry>,
}

impl Config {
    pub fn new(toml_string: &str) -> Config {
        let mut config = Config {
            entries: Vec::new(),
        };
        let config_table = toml_string.parse::<Table>().unwrap();
        for (key, value) in config_table.iter() {
            // check value for "template" and "destination"
            config.entries.push(ConfigEntry {
                name: key.to_string(),
                template_path: shellexpand::tilde(&value["template"].as_str().unwrap()).to_string(),
                destination_path: shellexpand::tilde(&value["destination"].as_str().unwrap())
                    .to_string(),
                post_hook: value
                    .get("post_hook")
                    .and_then(|v| v.as_str().map(|s| s.to_string())),
            });
        }

        return config;
    }
}
