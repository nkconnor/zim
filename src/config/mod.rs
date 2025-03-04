use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

mod key_bindings;
pub use key_bindings::KeyBindings;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default = "default_tab_size")]
    pub tab_size: usize,
    #[serde(default = "default_line_numbers")]
    pub line_numbers: bool,
    #[serde(default = "default_wrap_text")]
    pub wrap_text: bool,
    #[serde(default)]
    pub key_bindings: KeyBindings,
}

fn default_tab_size() -> usize { 4 }
fn default_line_numbers() -> bool { true }
fn default_wrap_text() -> bool { true }

#[derive(Debug, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_foreground")]
    pub foreground: String,
    #[serde(default = "default_selection")]
    pub selection: String,
    #[serde(default = "default_cursor")]
    pub cursor: String,
    #[serde(default = "default_status_line_bg")]
    pub status_line_bg: String,
    #[serde(default = "default_status_line_fg")]
    pub status_line_fg: String,
}

fn default_background() -> String { "#282c34".to_string() }
fn default_foreground() -> String { "#abb2bf".to_string() }
fn default_selection() -> String { "#3e4451".to_string() }
fn default_cursor() -> String { "#528bff".to_string() }
fn default_status_line_bg() -> String { "#4b5263".to_string() }
fn default_status_line_fg() -> String { "#abb2bf".to_string() }

// The default implementations now use the default functions we defined above
impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            tab_size: default_tab_size(),
            line_numbers: default_line_numbers(),
            wrap_text: default_wrap_text(),
            key_bindings: KeyBindings::default(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: default_background(),
            foreground: default_foreground(),
            selection: default_selection(),
            cursor: default_cursor(),
            status_line_bg: default_status_line_bg(),
            status_line_fg: default_status_line_fg(),
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