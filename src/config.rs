use std::path::Path;

pub struct ToolConfig {
    name: String,
    template_path: Box<Path>,
    destination_path: Box<Path>,
}

pub struct Config {
    tool_configs: Vec<ToolConfig>,
}

impl Config {
    // TODO: parse config from toml file
    pub fn from_toml(toml: &str) -> Config {
        Config {
            tool_configs: Vec::new(),
        }
    }
}
