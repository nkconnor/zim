use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyBinding {
    pub key: String,
    pub modifiers: Vec<String>,
}

impl KeyBinding {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            modifiers: Vec::new(),
        }
    }

    pub fn with_modifier(mut self, modifier: &str) -> Self {
        self.modifiers.push(modifier.to_string());
        self
    }

    pub fn matches(&self, event: &KeyEvent) -> bool {
        let key_matches = match event.code {
            KeyCode::Char(c) => self.key == c.to_string(),
            KeyCode::Enter => self.key == "enter",
            KeyCode::Tab => self.key == "tab",
            KeyCode::Backspace => self.key == "backspace",
            KeyCode::Esc => self.key == "esc",
            KeyCode::Left => self.key == "left",
            KeyCode::Right => self.key == "right",
            KeyCode::Up => self.key == "up",
            KeyCode::Down => self.key == "down",
            KeyCode::Home => self.key == "home",
            KeyCode::End => self.key == "end",
            KeyCode::PageUp => self.key == "pageup",
            KeyCode::PageDown => self.key == "pagedown",
            KeyCode::Delete => self.key == "delete",
            KeyCode::Insert => self.key == "insert",
            KeyCode::F(n) => self.key == format!("f{}", n),
            _ => false,
        };

        // Check modifiers
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        let alt = event.modifiers.contains(KeyModifiers::ALT);
        let shift = event.modifiers.contains(KeyModifiers::SHIFT);

        let modifiers_match = if self.modifiers.is_empty() {
            !ctrl && !alt && !shift
        } else {
            self.modifiers.iter().all(|m| match m.as_str() {
                "ctrl" => ctrl,
                "alt" => alt,
                "shift" => shift,
                _ => false,
            })
        };

        key_matches && modifiers_match
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyBindings {
    // Maps from command name to key binding
    #[serde(default)]
    pub normal_mode: HashMap<String, KeyBinding>,
    #[serde(default)]
    pub insert_mode: HashMap<String, KeyBinding>,
    #[serde(default)]
    pub command_mode: HashMap<String, KeyBinding>,
    #[serde(default)]
    pub file_finder_mode: HashMap<String, KeyBinding>,
    #[serde(default)]
    pub help_mode: HashMap<String, KeyBinding>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut normal_mode = HashMap::new();
        normal_mode.insert("quit".to_string(), KeyBinding::new("q"));
        normal_mode.insert("insert_mode".to_string(), KeyBinding::new("i"));
        normal_mode.insert("command_mode".to_string(), KeyBinding::new(":"));
        normal_mode.insert("move_left".to_string(), KeyBinding::new("h"));
        normal_mode.insert("move_down".to_string(), KeyBinding::new("j"));
        normal_mode.insert("move_up".to_string(), KeyBinding::new("k"));
        normal_mode.insert("move_right".to_string(), KeyBinding::new("l"));
        normal_mode.insert("find_file".to_string(), KeyBinding::new("p").with_modifier("ctrl"));
        
        // Line navigation
        normal_mode.insert("move_to_line_start".to_string(), KeyBinding::new("^"));
        normal_mode.insert("move_to_line_end".to_string(), KeyBinding::new("$"));
        
        // File navigation - using simple keys for now
        normal_mode.insert("move_to_file_start".to_string(), KeyBinding::new("g"));
        normal_mode.insert("move_to_file_end".to_string(), KeyBinding::new("G"));
        
        // Page navigation
        normal_mode.insert("page_up".to_string(), KeyBinding::new("b").with_modifier("ctrl"));
        normal_mode.insert("page_down".to_string(), KeyBinding::new("f").with_modifier("ctrl"));
        
        // Diagnostics
        normal_mode.insert("run_cargo_check".to_string(), KeyBinding::new("d").with_modifier("ctrl"));
        normal_mode.insert("run_cargo_clippy".to_string(), KeyBinding::new("y").with_modifier("ctrl"));
        
        // Tab management
        normal_mode.insert("new_tab".to_string(), KeyBinding::new("t").with_modifier("ctrl"));
        normal_mode.insert("close_tab".to_string(), KeyBinding::new("w").with_modifier("ctrl"));
        normal_mode.insert("next_tab".to_string(), KeyBinding::new("tab"));
        normal_mode.insert("prev_tab".to_string(), KeyBinding::new("tab").with_modifier("shift"));
        
        // Help
        normal_mode.insert("show_help".to_string(), KeyBinding::new("h").with_modifier("ctrl"));
        
        let mut insert_mode = HashMap::new();
        insert_mode.insert("normal_mode".to_string(), KeyBinding::new("esc"));

        let mut command_mode = HashMap::new();
        command_mode.insert("normal_mode".to_string(), KeyBinding::new("esc"));
        
        let mut help_mode = HashMap::new();
        help_mode.insert("normal_mode".to_string(), KeyBinding::new("esc"));

        let mut file_finder_mode = HashMap::new();
        file_finder_mode.insert("cancel".to_string(), KeyBinding::new("esc"));
        file_finder_mode.insert("select".to_string(), KeyBinding::new("enter"));
        file_finder_mode.insert("next".to_string(), KeyBinding::new("down"));
        file_finder_mode.insert("previous".to_string(), KeyBinding::new("up"));

        Self {
            normal_mode,
            insert_mode,
            command_mode,
            file_finder_mode,
            help_mode,
        }
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .with_context(|| "Failed to determine config directory")?
        .join("zim");
    
    Ok(config_dir)
}

impl KeyBindings {
    pub fn load() -> Result<Self> {
        let config_dir = get_config_dir()?;
        let bindings_path = config_dir.join("key_bindings.toml");

        if bindings_path.exists() {
            let bindings_str = fs::read_to_string(&bindings_path)
                .with_context(|| format!("Failed to read key bindings file: {:?}", bindings_path))?;
            
            let bindings = toml::from_str(&bindings_str)
                .with_context(|| format!("Failed to parse key bindings file: {:?}", bindings_path))?;
            
            Ok(bindings)
        } else {
            // Create default bindings
            let bindings = KeyBindings::default();
            
            // Ensure config directory exists
            fs::create_dir_all(&config_dir)
                .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
            
            // Write default bindings
            let bindings_str = toml::to_string_pretty(&bindings)
                .with_context(|| "Failed to serialize key bindings")?;
            
            fs::write(&bindings_path, bindings_str)
                .with_context(|| format!("Failed to write key bindings file: {:?}", bindings_path))?;
            
            Ok(bindings)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = get_config_dir()?;
        let bindings_path = config_dir.join("key_bindings.toml");

        // Ensure config directory exists
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
        
        // Write bindings
        let bindings_str = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize key bindings")?;
        
        fs::write(&bindings_path, bindings_str)
            .with_context(|| format!("Failed to write key bindings file: {:?}", bindings_path))?;
        
        Ok(())
    }
}