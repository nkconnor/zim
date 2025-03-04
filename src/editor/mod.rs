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
pub use diagnostics::{Diagnostic, DiagnosticSeverity, DiagnosticCollection};

use anyhow::Result;
use crossterm::event::KeyEvent;
use crate::config::Config;

pub struct Editor {
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub mode: Mode,
    pub file_finder: FileFinder,
    pub config: Config,
    pub viewport: Viewport,
    pub diagnostics: DiagnosticCollection,
}

impl Editor {
    pub fn new() -> Self {
        // Create with default config
        Self::new_with_config(Config::default())
    }

    pub fn new_with_config(config: Config) -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::new(),
            mode: Mode::Normal,
            file_finder: FileFinder::new(),
            config,
            viewport: Viewport::new(),
            diagnostics: DiagnosticCollection::new(),
        }
    }
    
    /// Run cargo check and parse the diagnostics
    pub fn run_cargo_check(&mut self, path: &str) -> Result<()> {
        use std::process::Command;
        
        // Run cargo check
        let output = Command::new("cargo")
            .arg("check")
            .arg("--message-format=human")
            .current_dir(path)
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Parse the diagnostics
        self.diagnostics.parse_cargo_output(&format!("{}\n{}", stdout, stderr), path);
        
        Ok(())
    }
    
    /// Run cargo clippy and parse the diagnostics
    pub fn run_cargo_clippy(&mut self, path: &str) -> Result<()> {
        use std::process::Command;
        
        // Run cargo clippy
        let output = Command::new("cargo")
            .arg("clippy")
            .arg("--message-format=human")
            .current_dir(path)
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Parse the diagnostics
        self.diagnostics.parse_cargo_output(&format!("{}\n{}", stdout, stderr), path);
        
        Ok(())
    }
    
    // Update the viewport if cursor moves out of the visible area
    pub fn update_viewport(&mut self) {
        self.viewport.ensure_cursor_visible(self.cursor.y, self.cursor.x);
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        let result = self.buffer.load_file(path);
        
        // Reset cursor and viewport
        self.cursor.x = 0;
        self.cursor.y = 0;
        self.viewport.top_line = 0;
        self.viewport.left_column = 0;
        
        result
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Command => self.handle_command_mode(key),
            Mode::FileFinder => self.handle_file_finder_mode(key),
        }
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
                        self.cursor.move_left(&self.buffer);
                        self.update_viewport();
                    },
                    "move_down" => {
                        self.cursor.move_down(&self.buffer);
                        self.update_viewport();
                    },
                    "move_up" => {
                        self.cursor.move_up(&self.buffer);
                        self.update_viewport();
                    },
                    "move_right" => {
                        self.cursor.move_right(&self.buffer);
                        self.update_viewport();
                    },
                    "move_to_line_start" => {
                        self.cursor.move_to_line_start(&self.buffer);
                        self.update_viewport();
                    },
                    "move_to_line_end" => {
                        self.cursor.move_to_line_end(&self.buffer);
                        self.update_viewport();
                    },
                    "move_to_file_start" => {
                        self.cursor.move_to_file_start(&self.buffer);
                        self.update_viewport();
                    },
                    "move_to_file_end" => {
                        self.cursor.move_to_file_end(&self.buffer);
                        self.update_viewport();
                    },
                    "page_up" => {
                        // Move cursor up by viewport height
                        let page_size = self.viewport.height.max(1);
                        for _ in 0..page_size {
                            if self.cursor.y > 0 {
                                self.cursor.move_up(&self.buffer);
                            } else {
                                break;
                            }
                        }
                        self.update_viewport();
                    },
                    "page_down" => {
                        // Move cursor down by viewport height
                        let page_size = self.viewport.height.max(1);
                        for _ in 0..page_size {
                            if self.cursor.y < self.buffer.line_count() - 1 {
                                self.cursor.move_down(&self.buffer);
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
            KeyCode::Char('h') => {
                self.cursor.move_left(&self.buffer);
                self.update_viewport();
            },
            KeyCode::Char('j') => {
                self.cursor.move_down(&self.buffer);
                self.update_viewport();
            },
            KeyCode::Char('k') => {
                self.cursor.move_up(&self.buffer);
                self.update_viewport();
            },
            KeyCode::Char('l') => {
                self.cursor.move_right(&self.buffer);
                self.update_viewport();
            },
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::FileFinder;
                self.file_finder.refresh()?;
            },
            // Beginning of line (^)
            KeyCode::Char('^') => {
                self.cursor.move_to_line_start(&self.buffer);
                self.update_viewport();
            },
            // End of line ($)
            KeyCode::Char('$') => {
                self.cursor.move_to_line_end(&self.buffer);
                self.update_viewport();
            },
            // Top of file (g) - in the future, might want to implement double-g
            KeyCode::Char('g') => {
                self.cursor.move_to_file_start(&self.buffer);
                self.update_viewport();
            },
            // Bottom of file (G)
            KeyCode::Char('G') => {
                self.cursor.move_to_file_end(&self.buffer);
                self.update_viewport();
            },
            // Page up (Ctrl+b)
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let page_size = self.viewport.height.max(1);
                for _ in 0..page_size {
                    if self.cursor.y > 0 {
                        self.cursor.move_up(&self.buffer);
                    } else {
                        break;
                    }
                }
                self.update_viewport();
            },
            // Page down (Ctrl+f)
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let page_size = self.viewport.height.max(1);
                for _ in 0..page_size {
                    if self.cursor.y < self.buffer.line_count() - 1 {
                        self.cursor.move_down(&self.buffer);
                    } else {
                        break;
                    }
                }
                self.update_viewport();
            },
            // Cargo check (Ctrl+c)
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let current_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .to_string();
                let _ = self.run_cargo_check(&current_dir);
            },
            // Cargo clippy (Ctrl+l)
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let current_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .to_string();
                let _ = self.run_cargo_clippy(&current_dir);
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
                self.buffer.insert_char_at_cursor(c, &self.cursor);
                self.cursor.move_right(&self.buffer);
                self.update_viewport();
            }
            KeyCode::Backspace => {
                if self.cursor.x > 0 {
                    self.cursor.move_left(&self.buffer);
                    self.buffer.delete_char_at_cursor(&self.cursor);
                    self.update_viewport();
                }
            }
            KeyCode::Enter => {
                self.buffer.insert_newline_at_cursor(&self.cursor);
                self.cursor.x = 0;
                self.cursor.y += 1;
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