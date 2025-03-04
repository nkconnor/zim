use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

mod key_bindings;
pub use key_bindings::KeyBindings;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub theme: Theme,
    pub tab_size: usize,
    pub line_numbers: bool,
    pub wrap_text: bool,
    pub key_bindings: KeyBindings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Theme {
    pub background: String,
    pub foreground: String,
    pub selection: String,
    pub cursor: String,
    pub status_line_bg: String,
    pub status_line_fg: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            tab_size: 4,
            line_numbers: true,
            wrap_text: true,
            key_bindings: KeyBindings::default(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: "#282c34".to_string(),
            foreground: "#abb2bf".to_string(),
            selection: "#3e4451".to_string(),
            cursor: "#528bff".to_string(),
            status_line_bg: "#4b5263".to_string(),
            status_line_fg: "#abb2bf".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_dir = get_config_dir()?;
        let config_path = config_dir.join("config.toml");

        if config_path.exists() {
            let config_str = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
            
            let config = toml::from_str(&config_str)
                .with_context(|| format!("Failed to parse config file: {:?}", config_path))?;
            
            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            
            // Ensure config directory exists
            fs::create_dir_all(&config_dir)
                .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
            
            // Write default config
            let config_str = toml::to_string_pretty(&config)
                .with_context(|| "Failed to serialize config")?;
            
            fs::write(&config_path, config_str)
                .with_context(|| format!("Failed to write config file: {:?}", config_path))?;
            
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = get_config_dir()?;
        let config_path = config_dir.join("config.toml");

        // Ensure config directory exists
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
        
        // Write config
        let config_str = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;
        
        fs::write(&config_path, config_str)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;
        
        Ok(())
    }
}

fn get_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .with_context(|| "Failed to determine config directory")?
        .join("zim");
    
    Ok(config_dir)
}