mod buffer;
mod cursor;
mod mode;
mod file_finder;
mod viewport;
mod diagnostics;

pub use buffer::Buffer;
pub use cursor::Cursor;
pub use mode::Mode;
pub use file_finder::FileFinder;
pub use viewport::Viewport;
pub use diagnostics::{DiagnosticSeverity, DiagnosticCollection};

use anyhow::Result;
use crossterm::event::KeyEvent;
use crate::config::Config;

/// Represents an editor tab with its own buffer, cursor, and viewport
pub struct Tab {
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub viewport: Viewport,
    pub diagnostics: DiagnosticCollection,
}

impl Tab {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::new(),
            viewport: Viewport::new(),
            diagnostics: DiagnosticCollection::new(),
        }
    }
    
    pub fn load_file(&mut self, path: &str) -> Result<()> {
        self.buffer.load_file(path)
    }
}

pub struct Editor {
    pub tabs: Vec<Tab>,
    pub current_tab: usize,
    pub mode: Mode,
    pub file_finder: FileFinder,
    pub config: Config,
}

impl Editor {
    pub fn new() -> Self {
        // Create with default config
        Self::new_with_config(Config::default())
    }

    pub fn new_with_config(config: Config) -> Self {
        // Create a default tab
        let mut tabs = Vec::new();
        tabs.push(Tab::new());
        
        Self {
            tabs,
            current_tab: 0,
            mode: Mode::Normal,
            file_finder: FileFinder::new(),
            config,
        }
    }
    
    /// Get a reference to the current tab
    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.current_tab]
    }
    
    /// Get a mutable reference to the current tab
    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.current_tab]
    }
    
    /// Add a new tab with an empty buffer
    pub fn add_tab(&mut self) {
        self.tabs.push(Tab::new());
        self.current_tab = self.tabs.len() - 1;
    }
    
    /// Switch to the next tab
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.current_tab = (self.current_tab + 1) % self.tabs.len();
        }
    }
    
    /// Switch to the previous tab
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.current_tab = if self.current_tab == 0 {
                self.tabs.len() - 1
            } else {
                self.current_tab - 1
            };
        }
    }
    
    /// Close the current tab
    pub fn close_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.tabs.remove(self.current_tab);
            
            // Adjust current_tab if it's now out of bounds
            if self.current_tab >= self.tabs.len() {
                self.current_tab = self.tabs.len() - 1;
            }
        }
    }
    
    /// Run cargo check and parse the diagnostics
    pub fn run_cargo_check(&mut self, cargo_dir: &str) -> Result<()> {
        use std::process::Command;
        
        // Get the current file path for diagnostic scoping
        let current_file = match &self.current_tab_mut().buffer.file_path {
            Some(path) => path.clone(),
            None => return Ok(()) // Can't run diagnostics without a file
        };
        
        // Run cargo check
        let output = Command::new("cargo")
            .arg("check")
            .arg("--message-format=human")
            .current_dir(cargo_dir)
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Parse the diagnostics, scoping to the current file
        self.current_tab_mut().diagnostics.parse_cargo_output(&format!("{}\n{}", stdout, stderr), &current_file);
        
        Ok(())
    }
    
    /// Run cargo clippy and parse the diagnostics
    pub fn run_cargo_clippy(&mut self, cargo_dir: &str) -> Result<()> {
        use std::process::Command;
        
        // Get the current file path for diagnostic scoping
        let current_file = match &self.current_tab_mut().buffer.file_path {
            Some(path) => path.clone(),
            None => return Ok(()) // Can't run diagnostics without a file
        };
        
        // Run cargo clippy
        let output = Command::new("cargo")
            .arg("clippy")
            .arg("--message-format=human")
            .current_dir(cargo_dir)
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Parse the diagnostics, scoping to the current file
        self.current_tab_mut().diagnostics.parse_cargo_output(&format!("{}\n{}", stdout, stderr), &current_file);
        
        Ok(())
    }
    
    // Update the viewport if cursor moves out of the visible area
    pub fn update_viewport(&mut self) {
        let tab = self.current_tab_mut();
        tab.viewport.ensure_cursor_visible(tab.cursor.y, tab.cursor.x);
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        let tab = self.current_tab_mut();
        
        let result = tab.buffer.load_file(path);
        
        // Reset cursor and viewport
        tab.cursor.x = 0;
        tab.cursor.y = 0;
        tab.viewport.top_line = 0;
        tab.viewport.left_column = 0;
        
        result
    }
    
    /// Load file in a new tab
    pub fn load_file_in_new_tab(&mut self, path: &str) -> Result<()> {
        self.add_tab();
        self.load_file(path)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Command => self.handle_command_mode(key),
            Mode::FileFinder => self.handle_file_finder_mode(key),
            Mode::Help => self.handle_help_mode(key),
        }
    }
    
    fn handle_help_mode(&mut self, key: KeyEvent) -> Result<bool> {
        let bindings = &self.config.key_bindings.help_mode;
        
        // Check bindings first
        for (command, binding) in bindings {
            if binding.matches(&key) {
                match command.as_str() {
                    "normal_mode" => self.mode = Mode::Normal,
                    _ => {}
                }
                return Ok(true);
            }
        }
        
        // Default handling
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('q') => self.mode = Mode::Normal,
            _ => {}
        }
        
        Ok(true)
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<bool> {
        let bindings = &self.config.key_bindings.normal_mode;

        // Check each command binding
        for (command, binding) in bindings {
            if binding.matches(&key) {
                match command.as_str() {
                    "quit" => return Ok(false),
                    "insert_mode" => self.mode = Mode::Insert,
                    "command_mode" => self.mode = Mode::Command,
                    "move_left" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_left(&tab.buffer);
                        self.update_viewport();
                    },
                    "move_down" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_down(&tab.buffer);
                        self.update_viewport();
                    },
                    "move_up" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_up(&tab.buffer);
                        self.update_viewport();
                    },
                    "move_right" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_right(&tab.buffer);
                        self.update_viewport();
                    },
                    "move_to_line_start" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_to_line_start(&tab.buffer);
                        self.update_viewport();
                    },
                    "move_to_line_end" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_to_line_end(&tab.buffer);
                        self.update_viewport();
                    },
                    "move_to_file_start" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_to_file_start(&tab.buffer);
                        self.update_viewport();
                    },
                    "move_to_file_end" => {
                        let tab = self.current_tab_mut();
                        tab.cursor.move_to_file_end(&tab.buffer);
                        self.update_viewport();
                    },
                    "page_up" => {
                        // Move cursor up by viewport height
                        let tab = self.current_tab_mut();
                        let page_size = tab.viewport.height.max(1);
                        for _ in 0..page_size {
                            if tab.cursor.y > 0 {
                                tab.cursor.move_up(&tab.buffer);
                            } else {
                                break;
                            }
                        }
                        self.update_viewport();
                    },
                    "page_down" => {
                        // Move cursor down by viewport height
                        let tab = self.current_tab_mut();
                        let page_size = tab.viewport.height.max(1);
                        for _ in 0..page_size {
                            if tab.cursor.y < tab.buffer.line_count() - 1 {
                                tab.cursor.move_down(&tab.buffer);
                            } else {
                                break;
                            }
                        }
                        self.update_viewport();
                    },
                    "run_cargo_check" => {
                        // Get current directory
                        let current_dir = std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("."))
                            .to_string_lossy()
                            .to_string();
                        
                        // Run cargo check (ignoring errors)
                        let _ = self.run_cargo_check(&current_dir);
                    },
                    "run_cargo_clippy" => {
                        // Get current directory
                        let current_dir = std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("."))
                            .to_string_lossy()
                            .to_string();
                        
                        // Run cargo clippy (ignoring errors)
                        let _ = self.run_cargo_clippy(&current_dir);
                    },
                    "new_tab" => {
                        self.add_tab();
                    },
                    "close_tab" => {
                        self.close_tab();
                    },
                    "next_tab" => {
                        self.next_tab();
                    },
                    "prev_tab" => {
                        self.prev_tab();
                    },
                    "show_help" => {
                        self.mode = Mode::Help;
                    },
                    "find_file" => {
                        self.mode = Mode::FileFinder;
                        self.file_finder.refresh()?;
                    }
                    _ => {}
                }
                return Ok(true);
            }
        }

        // Fall back to default handling if no binding matches
        use crossterm::event::{KeyCode, KeyModifiers};
        match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('i') => self.mode = Mode::Insert,
            KeyCode::Char(':') => self.mode = Mode::Command,
            KeyCode::Char('h') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let tab = self.current_tab_mut();
                tab.cursor.move_left(&tab.buffer);
                self.update_viewport();
            },
            KeyCode::Char('j') => {
                let tab = self.current_tab_mut();
                tab.cursor.move_down(&tab.buffer);
                self.update_viewport();
            },
            KeyCode::Char('k') => {
                let tab = self.current_tab_mut();
                tab.cursor.move_up(&tab.buffer);
                self.update_viewport();
            },
            KeyCode::Char('l') => {
                let tab = self.current_tab_mut();
                tab.cursor.move_right(&tab.buffer);
                self.update_viewport();
            },
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::FileFinder;
                self.file_finder.refresh()?;
            },
            // Beginning of line (^)
            KeyCode::Char('^') => {
                let tab = self.current_tab_mut();
                tab.cursor.move_to_line_start(&tab.buffer);
                self.update_viewport();
            },
            // End of line ($)
            KeyCode::Char('$') => {
                let tab = self.current_tab_mut();
                tab.cursor.move_to_line_end(&tab.buffer);
                self.update_viewport();
            },
            // Top of file (g) - in the future, might want to implement double-g
            KeyCode::Char('g') => {
                let tab = self.current_tab_mut();
                tab.cursor.move_to_file_start(&tab.buffer);
                self.update_viewport();
            },
            // Bottom of file (G)
            KeyCode::Char('G') => {
                let tab = self.current_tab_mut();
                tab.cursor.move_to_file_end(&tab.buffer);
                self.update_viewport();
            },
            // Page up (Ctrl+b)
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let tab = self.current_tab_mut();
                let page_size = tab.viewport.height.max(1);
                for _ in 0..page_size {
                    if tab.cursor.y > 0 {
                        tab.cursor.move_up(&tab.buffer);
                    } else {
                        break;
                    }
                }
                self.update_viewport();
            },
            // Page down (Ctrl+f)
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let tab = self.current_tab_mut();
                let page_size = tab.viewport.height.max(1);
                for _ in 0..page_size {
                    if tab.cursor.y < tab.buffer.line_count() - 1 {
                        tab.cursor.move_down(&tab.buffer);
                    } else {
                        break;
                    }
                }
                self.update_viewport();
            },
            // Cargo check (Ctrl+d)
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let current_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .to_string();
                let _ = self.run_cargo_check(&current_dir);
            },
            // Cargo clippy (Ctrl+y)
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let current_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .to_string();
                let _ = self.run_cargo_clippy(&current_dir);
            },
            // Help (Ctrl+h)
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::Help;
            },
            _ => {}
        }

        Ok(true)
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<bool> {
        let bindings = &self.config.key_bindings.insert_mode;

        // Check bindings first
        for (command, binding) in bindings {
            if binding.matches(&key) {
                match command.as_str() {
                    "normal_mode" => self.mode = Mode::Normal,
                    _ => {}
                }
                return Ok(true);
            }
        }

        // Default handling for insert mode
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char(c) => {
                let tab = self.current_tab_mut();
                tab.buffer.insert_char_at_cursor(c, &tab.cursor);
                tab.cursor.move_right(&tab.buffer);
                self.update_viewport();
            }
            KeyCode::Backspace => {
                let tab = self.current_tab_mut();
                if tab.cursor.x > 0 {
                    tab.cursor.move_left(&tab.buffer);
                    tab.buffer.delete_char_at_cursor(&tab.cursor);
                    self.update_viewport();
                }
            }
            KeyCode::Enter => {
                let tab = self.current_tab_mut();
                tab.buffer.insert_newline_at_cursor(&tab.cursor);
                tab.cursor.x = 0;
                tab.cursor.y += 1;
                self.update_viewport();
            }
            _ => {}
        }

        Ok(true)
    }

    fn handle_command_mode(&mut self, key: KeyEvent) -> Result<bool> {
        let bindings = &self.config.key_bindings.command_mode;

        // Check bindings first
        for (command, binding) in bindings {
            if binding.matches(&key) {
                match command.as_str() {
                    "normal_mode" => self.mode = Mode::Normal,
                    _ => {}
                }
                return Ok(true);
            }
        }

        // Default handling
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            _ => {}
        }

        Ok(true)
    }

    fn handle_file_finder_mode(&mut self, key: KeyEvent) -> Result<bool> {
        let bindings = &self.config.key_bindings.file_finder_mode;

        // Check bindings first
        for (command, binding) in bindings {
            if binding.matches(&key) {
                match command.as_str() {
                    "cancel" => self.mode = Mode::Normal,
                    "select" => {
                        if let Some(file_path) = self.file_finder.get_selected() {
                            self.load_file(&file_path)?;
                            self.mode = Mode::Normal;
                        }
                    }
                    "next" => self.file_finder.next(),
                    "previous" => self.file_finder.previous(),
                    _ => {}
                }
                return Ok(true);
            }
        }

        // Default handling
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                if let Some(file_path) = self.file_finder.get_selected() {
                    self.load_file(&file_path)?;
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Down => self.file_finder.next(),
            KeyCode::Up => self.file_finder.previous(),
            KeyCode::Char(c) => {
                self.file_finder.add_char(c);
                self.file_finder.update_matches()?;
            }
            KeyCode::Backspace => {
                self.file_finder.remove_char();
                self.file_finder.update_matches()?;
            }
            _ => {}
        }

        Ok(true)
    }
}