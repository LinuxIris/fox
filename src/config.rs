use std::path::{Path, PathBuf};

use serde::Deserialize;

// TODO: Change this location, it's not very clean
pub fn config_location() -> std::io::Result<PathBuf> {
    std::env::current_exe().map(|p| p.parent().unwrap().to_owned()).map(|mut p| { p.push(Path::new("config.toml")); p })
}

#[derive(Deserialize)]
pub struct Config {
    pub theme: ConfigTheme,
}

#[derive(Deserialize)]
pub struct ConfigTheme {
    pub name: String,
    pub light_fix: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ConfigTheme {
                name: String::from("gruvbox-dark"),
                light_fix: false,
            }
        }
    }
}

pub fn config() -> Config {
    if let Ok(path) = config_location() {
        if let Ok(c) = std::fs::read_to_string(path) {
            toml::from_str(&c).unwrap_or(Config::default())
        } else {
            Config::default()
        }
    } else {
        Config::default()
    }
}
