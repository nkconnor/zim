use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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
    pub token_search_mode: HashMap<String, KeyBinding>,
    #[serde(default)]
    pub help_mode: HashMap<String, KeyBinding>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut normal_mode = HashMap::new();
        normal_mode.insert("quit".to_string(), KeyBinding::new("q"));
        normal_mode.insert("insert_mode".to_string(), KeyBinding::new("i"));
        // Direct file operations without command mode
        normal_mode.insert("save_file".to_string(), KeyBinding::new("w"));
        normal_mode.insert("reload_file".to_string(), KeyBinding::new("e"));
        normal_mode.insert("save_and_quit".to_string(), KeyBinding::new("X"));
        // We'll handle 'd' directly in normal mode for delete operations
        normal_mode.insert("delete_char".to_string(), KeyBinding::new("x"));
        normal_mode.insert("snake_game".to_string(), KeyBinding::new("s"));
        normal_mode.insert("open_line_below".to_string(), KeyBinding::new("o"));
        normal_mode.insert("open_line_above".to_string(), KeyBinding::new("O"));
        normal_mode.insert("move_left".to_string(), KeyBinding::new("h"));
        normal_mode.insert("move_down".to_string(), KeyBinding::new("j"));
        normal_mode.insert("move_up".to_string(), KeyBinding::new("k"));
        normal_mode.insert("move_right".to_string(), KeyBinding::new("l"));
        normal_mode.insert("undo".to_string(), KeyBinding::new("u"));
        normal_mode.insert("redo".to_string(), KeyBinding::new("r").with_modifier("ctrl"));
        normal_mode.insert(
            "find_file".to_string(),
            KeyBinding::new("o").with_modifier("ctrl"),
        );
        // Token search mode
        normal_mode.insert(
            "token_search".to_string(),
            KeyBinding::new("t").with_modifier("ctrl"),
        );

        // Line navigation
        normal_mode.insert("move_to_line_start".to_string(), KeyBinding::new("^"));
        normal_mode.insert("move_to_line_end".to_string(), KeyBinding::new("$"));

        // File navigation - using simple keys for now
        normal_mode.insert("move_to_file_start".to_string(), KeyBinding::new("g"));
        normal_mode.insert("move_to_file_end".to_string(), KeyBinding::new("G"));

        // Page navigation
        normal_mode.insert(
            "page_up".to_string(),
            KeyBinding::new("b").with_modifier("ctrl"),
        );
        normal_mode.insert(
            "page_down".to_string(),
            KeyBinding::new("f").with_modifier("ctrl"),
        );

        // Diagnostics
        normal_mode.insert(
            "run_cargo_check".to_string(),
            KeyBinding::new("d").with_modifier("ctrl"),
        );
        normal_mode.insert(
            "run_cargo_clippy".to_string(),
            KeyBinding::new("y").with_modifier("ctrl"),
        );

        // Tab management
        normal_mode.insert(
            "new_tab".to_string(),
            KeyBinding::new("n").with_modifier("ctrl"),
        );
        normal_mode.insert(
            "close_tab".to_string(),
            KeyBinding::new("w").with_modifier("ctrl"),
        );
        // Use only the key combinations that are confirmed to work reliably
        normal_mode.insert(
            "next_tab".to_string(),
            KeyBinding::new("right").with_modifier("ctrl"),
        );
        normal_mode.insert(
            "prev_tab".to_string(),
            KeyBinding::new("left").with_modifier("ctrl"),
        );

        // F-key navigation for tabs (1-12)
        normal_mode.insert("goto_tab_1".to_string(), KeyBinding::new("f1"));
        normal_mode.insert("goto_tab_2".to_string(), KeyBinding::new("f2"));
        normal_mode.insert("goto_tab_3".to_string(), KeyBinding::new("f3"));
        normal_mode.insert("goto_tab_4".to_string(), KeyBinding::new("f4"));
        normal_mode.insert("goto_tab_5".to_string(), KeyBinding::new("f5"));
        normal_mode.insert("goto_tab_6".to_string(), KeyBinding::new("f6"));
        normal_mode.insert("goto_tab_7".to_string(), KeyBinding::new("f7"));
        normal_mode.insert("goto_tab_8".to_string(), KeyBinding::new("f8"));
        normal_mode.insert("goto_tab_9".to_string(), KeyBinding::new("f9"));
        normal_mode.insert("goto_tab_10".to_string(), KeyBinding::new("f10"));
        normal_mode.insert("goto_tab_11".to_string(), KeyBinding::new("f11"));
        normal_mode.insert("goto_tab_12".to_string(), KeyBinding::new("f12"));

        // Help
        normal_mode.insert(
            "show_help".to_string(),
            KeyBinding::new("h").with_modifier("ctrl"),
        );

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

        let mut token_search_mode = HashMap::new();
        token_search_mode.insert("cancel".to_string(), KeyBinding::new("esc"));
        token_search_mode.insert("select".to_string(), KeyBinding::new("enter"));
        token_search_mode.insert("next".to_string(), KeyBinding::new("down"));
        token_search_mode.insert("previous".to_string(), KeyBinding::new("up"));

        Self {
            normal_mode,
            insert_mode,
            command_mode,
            file_finder_mode,
            token_search_mode,
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
        let config_dir = get_config_dir();
        
        // Just return default bindings if we can't get config directory
        if config_dir.is_err() {
            return Ok(KeyBindings::default());
        }
        
        let config_dir = config_dir?;
        let bindings_path = config_dir.join("key_bindings.toml");

        if bindings_path.exists() {
            // Try to read and parse existing bindings, fall back to default on error
            let bindings_str = match fs::read_to_string(&bindings_path) {
                Ok(s) => s,
                Err(_) => return Ok(KeyBindings::default()),
            };

            let bindings = match toml::from_str(&bindings_str) {
                Ok(b) => b,
                Err(_) => return Ok(KeyBindings::default()),
            };

            Ok(bindings)
        } else {
            // Create default bindings
            let bindings = KeyBindings::default();

            // Try to create config directory and file, but don't fail if we can't
            if let Err(_) = fs::create_dir_all(&config_dir) {
                return Ok(bindings);
            }

            // Format bindings to TOML
            let bindings_str = match toml::to_string_pretty(&bindings) {
                Ok(s) => s,
                Err(_) => return Ok(bindings),
            };

            // Write bindings file, but don't fail if we can't
            let _ = fs::write(&bindings_path, bindings_str);

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
        let bindings_str =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize key bindings")?;

        fs::write(&bindings_path, bindings_str)
            .with_context(|| format!("Failed to write key bindings file: {:?}", bindings_path))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_key_binding_matches() {
        // Test Ctrl+n for new tab
        let binding = KeyBinding::new("n").with_modifier("ctrl");

        // Should match Ctrl+n
        let ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert!(binding.matches(&ctrl_n));

        // Should not match Alt+n
        let alt_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT);
        assert!(!binding.matches(&alt_n));

        // Should not match just n
        let just_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        assert!(!binding.matches(&just_n));
    }

    #[test]
    fn test_default_key_bindings() {
        let bindings = KeyBindings::default();

        // Verify our new tab binding uses Ctrl+n
        if let Some(new_tab_binding) = bindings.normal_mode.get("new_tab") {
            assert_eq!(new_tab_binding.key, "n");
            assert!(new_tab_binding.modifiers.contains(&"ctrl".to_string()));
        } else {
            panic!("No binding found for new_tab");
        }
    }
}
