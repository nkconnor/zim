mod buffer;
mod cursor;
mod mode;
mod file_finder;
mod viewport;
mod diagnostics;
mod syntax;

pub use buffer::Buffer;
pub use cursor::Cursor;
pub use mode::Mode;
pub use file_finder::FileFinder;
pub use viewport::Viewport;
pub use diagnostics::{DiagnosticSeverity, DiagnosticCollection};
pub use syntax::{SyntaxHighlighter, HighlightedLine};

use anyhow::Result;
use crossterm::event::KeyEvent;
use crate::config::Config;
use std::collections::{HashSet, HashMap};

/// Represents a command that can be executed in the editor
/// 
/// This enum implements a command pattern for editor operations,
/// making it easier to map key bindings to actions and to
/// reuse common operations throughout the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand {
    // File operations
    SaveFile,
    ReloadFile,
    SaveAndQuit,
    Quit,
    
    // Mode switching
    EnterNormalMode,
    EnterInsertMode,
    EnterCommandMode,
    EnterFileFinder,
    ShowHelp,
    
    // Navigation
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveToLineStart,
    MoveToLineEnd,
    MoveToFileStart,
    MoveToFileEnd,
    PageUp,
    PageDown,
    
    // Tab operations
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    GotoTab(usize),
    
    // Rust integration
    RunCargoCheck,
    RunCargoClippy,
    
    // No operation
    Noop,
}

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
    
    pub fn new_with_name(name: &str) -> Self {
        let mut tab = Self::new();
        tab.buffer.file_path = Some(name.to_string());
        tab
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
    pub save_and_quit: bool,
    pub command_text: String,
    pub filename_prompt_text: String,
    pub diff_lines: HashSet<usize>,
    pub syntax_highlighter: SyntaxHighlighter,
    /// Cache of highlighted lines to avoid recomputing syntax highlighting on every render
    pub highlighted_lines_cache: HashMap<(usize, usize), Vec<HighlightedLine>>,
    /// Clipboard for storing yanked/copied text
    pub clipboard: String,
}

impl Editor {
    pub fn new() -> Self {
        // Create with default config
        Self::new_with_config(Config::default())
    }

    pub fn new_with_config(config: Config) -> Self {
        // Create a default tab with a name
        let mut tabs = Vec::new();
        tabs.push(Tab::new_with_name("untitled-1"));
        
        // Initialize with file finder mode to show welcome screen
        let mut editor = Self {
            tabs,
            current_tab: 0,
            mode: Mode::FileFinder,
            file_finder: FileFinder::new(),
            config,
            save_and_quit: false,
            command_text: String::new(),
            filename_prompt_text: String::new(),
            diff_lines: HashSet::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            highlighted_lines_cache: HashMap::new(),
            clipboard: String::new(),
        };
        
        // Refresh file finder to populate files list
        let _ = editor.file_finder.refresh();
        
        editor
    }
    
    /// Invalidate the syntax highlighting cache when a buffer is modified
    pub fn invalidate_highlight_cache(&mut self) {
        self.highlighted_lines_cache.clear();
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
        // Generate a unique name for the new tab
        let tab_number = self.tabs.len() + 1;
        let tab_name = format!("untitled-{}", tab_number);
        
        self.tabs.push(Tab::new_with_name(&tab_name));
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
    
    /// Go to a specific tab by index (0-based)
    pub fn go_to_tab(&mut self, index: usize) {
        if !self.tabs.is_empty() && index < self.tabs.len() {
            self.current_tab = index;
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
    
    /// Execute a cargo command and process its diagnostics
/// 
/// This is a general-purpose function that can run any cargo command
/// and parse its output for diagnostics. It reduces code duplication.
pub fn run_cargo_command(&mut self, cargo_dir: &str, command: &str) -> Result<()> {
    use std::process::Command;
    
    // Get the current file path for diagnostic scoping
    let current_file = match &self.current_tab_mut().buffer.file_path {
        Some(path) => path.clone(),
        None => return Ok(()) // Can't run diagnostics without a file
    };
    
    // Run the cargo command
    let output = Command::new("cargo")
        .arg(command)
        .arg("--message-format=human")
        .current_dir(cargo_dir)
        .output()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Parse the diagnostics, scoping to the current file
    self.current_tab_mut().diagnostics.parse_cargo_output(&format!("{}\n{}", stdout, stderr), &current_file);
    
    Ok(())
}

/// Run cargo check and parse the diagnostics
///
/// This function runs the `cargo check` command in the specified directory
/// and parses any diagnostics (errors, warnings) that are produced,
/// associating them with the current file.
/// 
/// # Arguments
/// 
/// * `cargo_dir` - The directory where cargo should be run
/// 
/// # Returns
/// 
/// * `Result<()>` - Ok if the command succeeds, or an error if it fails
pub fn run_cargo_check(&mut self, cargo_dir: &str) -> Result<()> {
    if let Err(e) = self.run_cargo_command(cargo_dir, "check") {
        // Provide a more user-friendly error message
        return Err(anyhow::anyhow!("Failed to run cargo check: {}", e));
    }
    Ok(())
}

/// Run cargo clippy and parse the diagnostics
///
/// This function runs the `cargo clippy` command in the specified directory
/// and parses any lints (style warnings, suggestions) that are produced,
/// associating them with the current file.
/// 
/// # Arguments
/// 
/// * `cargo_dir` - The directory where cargo should be run
/// 
/// # Returns
/// 
/// * `Result<()>` - Ok if the command succeeds, or an error if it fails
pub fn run_cargo_clippy(&mut self, cargo_dir: &str) -> Result<()> {
    if let Err(e) = self.run_cargo_command(cargo_dir, "clippy") {
        // Provide a more user-friendly error message
        return Err(anyhow::anyhow!("Failed to run cargo clippy: {}", e));
    }
    Ok(())
}
    
    // Update the viewport if cursor moves out of the visible area
    pub fn update_viewport(&mut self) {
        let tab = self.current_tab_mut();
        tab.viewport.ensure_cursor_visible(tab.cursor.y, tab.cursor.x);
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        // First load the file
        let result = {
            let tab = self.current_tab_mut();
            let load_result = tab.buffer.load_file(path);
            
            // Reset cursor and viewport
            tab.cursor.x = 0;
            tab.cursor.y = 0;
            tab.viewport.top_line = 0;
            tab.viewport.left_column = 0;
            
            load_result
        };
        
        // Then determine syntax if load was successful
        if result.is_ok() {
            // Get the information needed for syntax determination
            let (file_path, first_line) = {
                let tab = self.current_tab();
                let file_path = tab.buffer.file_path.clone();
                let first_line = if !tab.buffer.lines.is_empty() {
                    tab.buffer.lines[0].clone()
                } else {
                    String::new()
                };
                (file_path, first_line)
            };
            
            // Determine syntax
            let syntax = self.syntax_highlighter.determine_syntax(
                file_path.as_deref(),
                &first_line
            );
            
            // Set the syntax
            self.current_tab_mut().buffer.set_syntax(syntax);
            
            // Add to recent files if we have a file path (clone to avoid borrowing issues)
            if let Some(file_path) = self.current_tab().buffer.file_path.clone() {
                self.file_finder.add_recent_file(&file_path);
            }
        }
        
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
            Mode::WriteConfirm => self.handle_write_confirm_mode(key),
            Mode::FilenamePrompt => self.handle_filename_prompt_mode(key),
            Mode::ReloadConfirm => self.handle_reload_confirm_mode(key),
            // Visual mode with character selection
            Mode::Visual => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Esc => {
                        self.current_tab_mut().buffer.clear_selection();
                        self.mode = Mode::Normal;
                        Ok(true)
                    },
                    // Delete selection
                    KeyCode::Char('d') => {
                        let is_deleted = {
                            let tab = self.current_tab_mut();
                            tab.buffer.delete_selection(&mut tab.cursor, false)
                        };
                        if is_deleted {
                            self.invalidate_highlight_cache();
                        }
                        self.mode = Mode::Normal;
                        Ok(true)
                    },
                    // Yank (copy) selection
                    KeyCode::Char('y') => {
                        // Get selected text
                        let selected_text = {
                            let tab = self.current_tab();
                            tab.buffer.get_selected_text(&tab.cursor, false)
                        };
                        
                        // Store in clipboard
                        self.clipboard = selected_text;
                        
                        // Clear selection and return to normal mode
                        self.current_tab_mut().buffer.clear_selection();
                        self.mode = Mode::Normal;
                        Ok(true)
                    },
                    // Add support for other key handling in visual mode
                    _ => self.handle_normal_mode(key),
                }
            },
            // Visual line mode with line selection
            Mode::VisualLine => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Esc => {
                        self.current_tab_mut().buffer.clear_selection();
                        self.mode = Mode::Normal;
                        Ok(true)
                    },
                    // Delete selection
                    KeyCode::Char('d') => {
                        let is_deleted = {
                            let tab = self.current_tab_mut();
                            tab.buffer.delete_selection(&mut tab.cursor, true)
                        };
                        if is_deleted {
                            self.invalidate_highlight_cache();
                        }
                        self.mode = Mode::Normal;
                        Ok(true)
                    },
                    // Yank (copy) selection
                    KeyCode::Char('y') => {
                        // Get selected text
                        let selected_text = {
                            let tab = self.current_tab();
                            tab.buffer.get_selected_text(&tab.cursor, true)
                        };
                        
                        // Store in clipboard
                        self.clipboard = selected_text;
                        
                        // Clear selection and return to normal mode
                        self.current_tab_mut().buffer.clear_selection();
                        self.mode = Mode::Normal;
                        Ok(true)
                    },
                    // Add support for other key handling in visual line mode
                    _ => self.handle_normal_mode(key),
                }
            },
        }
    }
    
    fn handle_write_confirm_mode(&mut self, key: KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;
        
        // Clone the current file path first to avoid borrow issues
        let current_path = self.current_tab().buffer.file_path.clone();
        let should_quit = self.save_and_quit;
        
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // User confirmed write operation
                if let Some(path) = current_path {
                    if path.starts_with("untitled-") {
                        // Need a real filename - enter filename prompt mode
                        self.filename_prompt_text.clear();
                        self.mode = Mode::FilenamePrompt;
                        return Ok(true);
                    } else {
                        if let Err(e) = self.current_tab_mut().buffer.save(None) {
                            // Stay in normal mode if there was an error
                            self.mode = Mode::Normal;
                            self.save_and_quit = false;
                            println!("Error saving file: {}", e);
                        } else {
                            // Show a success message
                            println!("File saved successfully: {}", path);
                            
                            // Add to recent files list
                            self.file_finder.add_recent_file(&path);
                            
                            // Check if we should quit after saving
                            if should_quit {
                                self.save_and_quit = false;
                                return Ok(false); // Exit the editor
                            }
                            
                            // Return to normal mode
                            self.mode = Mode::Normal;
                        }
                    }
                } else {
                    // No filename, enter filename prompt mode
                    self.filename_prompt_text.clear();
                    self.mode = Mode::FilenamePrompt;
                    return Ok(true);
                }
                Ok(true)
            },
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                // User cancelled write operation
                // Reset the save and quit flag
                self.save_and_quit = false;
                self.mode = Mode::Normal;
                Ok(true)
            },
            KeyCode::Char('q') => {
                // User wants to quit without saving
                self.save_and_quit = false;
                return Ok(false); // Exit the editor
            },
            _ => Ok(true), // Ignore other keys in write confirm mode
        }
    }
    
    fn handle_filename_prompt_mode(&mut self, key: KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;
        
        let should_quit = self.save_and_quit;
        
        match key.code {
            KeyCode::Esc => {
                // Cancel the filename prompt
                self.filename_prompt_text.clear();
                self.save_and_quit = false;
                self.mode = Mode::Normal;
            },
            KeyCode::Enter => {
                // Validate and save the file with the provided filename
                if !self.filename_prompt_text.trim().is_empty() {
                    let filename = self.filename_prompt_text.trim().to_string();
                    
                    // Save the file with the new name
                    if let Err(e) = self.current_tab_mut().buffer.save(Some(&filename)) {
                        println!("Error saving file: {}", e);
                    } else {
                        println!("File saved successfully: {}", filename);
                        
                        // Add to recent files
                        self.file_finder.add_recent_file(&filename);
                        
                        // Check if we should quit after saving
                        if should_quit {
                            self.save_and_quit = false;
                            return Ok(false); // Exit the editor
                        }
                    }
                } else {
                    println!("Error: Empty filename");
                }
                
                // Reset and return to normal mode
                self.filename_prompt_text.clear();
                self.mode = Mode::Normal;
            },
            KeyCode::Char(c) => {
                // Add the character to the filename
                self.filename_prompt_text.push(c);
            },
            KeyCode::Backspace => {
                // Remove the last character from the filename
                self.filename_prompt_text.pop();
            },
            _ => {}
        }
        
        Ok(true)
    }
    
    fn handle_reload_confirm_mode(&mut self, key: KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;
        
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // User confirmed reload
                if let Some(path) = &self.current_tab().buffer.file_path.clone() {
                    if !path.starts_with("untitled-") {
                        // Actually reload the file
                        // Simply attempt to reload the file
                        let _ = self.current_tab_mut().buffer.load_file(path);
                    }
                }
                
                // Clear diff lines and return to normal mode
                self.diff_lines.clear();
                self.mode = Mode::Normal;
            },
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                // User cancelled reload
                self.diff_lines.clear();
                self.mode = Mode::Normal;
            },
            _ => {
                // Ignore other keys in reload confirm mode
            }
        }
        
        Ok(true)
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
                    "save_file" => {
                        // Enter write confirmation mode with modified text highlighted
                        self.mode = Mode::WriteConfirm;
                        // Make sure save_and_quit flag is reset
                        self.save_and_quit = false;
                    },
                    "reload_file" => {
                        // Shortcut for reloading file (directly from normal mode)
                        if let Some(path) = &self.current_tab().buffer.file_path.clone() {
                            if !path.starts_with("untitled-") {
                                if let Err(e) = self.current_tab_mut().buffer.load_file(path) {
                                    println!("Error reloading file: {}", e);
                                }
                            }
                        }
                    },
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
                    "save_and_quit" => {
                        // Set flag so that the WriteConfirm handler knows to quit after saving
                        self.save_and_quit = true;
                        self.mode = Mode::WriteConfirm;
                    },
                    "next_tab" => {
                        self.next_tab();
                    },
                    "prev_tab" => {
                        self.prev_tab();
                    },
                    "goto_tab_1" => {
                        self.go_to_tab(0);
                    },
                    "goto_tab_2" => {
                        self.go_to_tab(1);
                    },
                    "goto_tab_3" => {
                        self.go_to_tab(2);
                    },
                    "goto_tab_4" => {
                        self.go_to_tab(3);
                    },
                    "goto_tab_5" => {
                        self.go_to_tab(4);
                    },
                    "goto_tab_6" => {
                        self.go_to_tab(5);
                    },
                    "goto_tab_7" => {
                        self.go_to_tab(6);
                    },
                    "goto_tab_8" => {
                        self.go_to_tab(7);
                    },
                    "goto_tab_9" => {
                        self.go_to_tab(8);
                    },
                    "goto_tab_10" => {
                        self.go_to_tab(9);
                    },
                    "goto_tab_11" => {
                        self.go_to_tab(10);
                    },
                    "goto_tab_12" => {
                        self.go_to_tab(11);
                    },
                    "show_help" => {
                        self.mode = Mode::Help;
                    },
                    "find_file" => {
                        self.mode = Mode::FileFinder;
                        self.file_finder.refresh()?;
                    },
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
            // Add explicit handling for w (write/save)
            KeyCode::Char('w') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Enter write confirmation mode
                self.mode = Mode::WriteConfirm;
                self.save_and_quit = false;
            },
            // Add explicit handling for e (edit/reload)
            KeyCode::Char('e') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::SHIFT) => {
                if let Some(path) = &self.current_tab().buffer.file_path.clone() {
                    if !path.starts_with("untitled-") {
                        // Calculate diff between buffer and disk
                        match self.current_tab().buffer.diff_with_disk() {
                            Ok(diff) => {
                                // Check if we found any differences
                                
                                if !diff.is_empty() {
                                    // Store diff lines for highlighting
                                    self.diff_lines = diff;
                                    // Enter reload confirmation mode
                                    self.mode = Mode::ReloadConfirm;
                                    
                                            // Enter reload confirm mode
                                } else {
                                    // No differences, no need to reload
                                    // Silent mode - no message needed
                                }
                            },
                            Err(_) => {
                                // Silent error handling - no UI output
                            }
                        }
                    }
                }
            },
            // Add explicit handling for X (save and quit) - capital X to avoid conflict with x
            KeyCode::Char('X') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.save_and_quit = true;
                self.mode = Mode::WriteConfirm;
            },
            // Add handling for d (delete line)
            KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::SHIFT) => {
                let cursor_y = self.current_tab().cursor.y;
                self.current_tab_mut().buffer.delete_line(cursor_y);
                
                // Adjust cursor if needed
                let tab = self.current_tab_mut();
                if tab.cursor.y >= tab.buffer.line_count() {
                    tab.cursor.y = tab.buffer.line_count().saturating_sub(1);
                }
                // Reset x position
                let line_len = tab.buffer.line_length(tab.cursor.y);
                if tab.cursor.x > line_len {
                    tab.cursor.x = line_len.saturating_sub(1).max(0);
                }
                
                self.update_viewport();
                self.invalidate_highlight_cache();
            },
            // v to enter visual mode
            KeyCode::Char('v') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.mode = Mode::Visual;
                // Start selection at current cursor position
                let current_pos = (self.current_tab().cursor.y, self.current_tab().cursor.x);
                self.current_tab_mut().buffer.start_selection(current_pos);
            },
            // V to enter visual line mode
            KeyCode::Char('V') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.mode = Mode::VisualLine;
                // Start selection at beginning of current line
                let current_line = self.current_tab().cursor.y;
                self.current_tab_mut().buffer.start_selection((current_line, 0));
            },
            // Add handling for x (delete character and enter insert mode) - to mirror vim
            KeyCode::Char('x') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::SHIFT) => {
                let tab = self.current_tab_mut();
                tab.buffer.delete_char_at_cursor(&tab.cursor);
                self.mode = Mode::Insert;
                self.update_viewport();
                self.invalidate_highlight_cache();
            },
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
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::FileFinder;
                self.file_finder.refresh()?;
            },
            // Paste clipboard after cursor (p key)
            KeyCode::Char('p') if !key.modifiers.contains(KeyModifiers::CONTROL) && 
                                   !key.modifiers.contains(KeyModifiers::ALT) && 
                                   !key.modifiers.contains(KeyModifiers::SHIFT) => {
                if !self.clipboard.is_empty() {
                    // Clone the clipboard content to avoid borrowing issues
                    let clipboard_content = self.clipboard.clone();
                    let ends_with_newline = clipboard_content.ends_with('\n');
                    
                    let tab = self.current_tab_mut();
                    let cursor_y = tab.cursor.y;
                    
                    // Check if clipboard ends with newline to determine paste style
                    if ends_with_newline {
                        // Paste on new line below current line
                        // First, find the last character of the current line
                        tab.cursor.move_to_line_end(&tab.buffer);
                        
                        // Insert a newline
                        tab.buffer.insert_newline_at_cursor(&tab.cursor);
                        
                        // Move to the beginning of the new line
                        tab.cursor.y += 1;
                        tab.cursor.x = 0;
                        
                        // Calculate clipboard lines
                        let clipboard_lines: Vec<&str> = clipboard_content.lines().collect();
                        
                        // Insert each line from the clipboard
                        for (i, line) in clipboard_lines.iter().enumerate() {
                            // Insert the line content
                            for c in line.chars() {
                                tab.buffer.insert_char_at_cursor(c, &tab.cursor);
                                tab.cursor.x += 1;
                            }
                            
                            // If not the last line, add a newline
                            if i < clipboard_lines.len() - 1 {
                                tab.buffer.insert_newline_at_cursor(&tab.cursor);
                                tab.cursor.y += 1;
                                tab.cursor.x = 0;
                            }
                        }
                        
                        // Position cursor at the start of the first pasted line
                        tab.cursor.y = cursor_y + 1;
                        tab.cursor.x = 0;
                    } else {
                        // Paste inline after cursor
                        for c in clipboard_content.chars() {
                            tab.buffer.insert_char_at_cursor(c, &tab.cursor);
                            tab.cursor.x += 1;
                        }
                    }
                    
                    self.update_viewport();
                    self.invalidate_highlight_cache();
                }
            },
            // Paste clipboard before cursor (P key)
            KeyCode::Char('P') if !key.modifiers.contains(KeyModifiers::CONTROL) && 
                                   !key.modifiers.contains(KeyModifiers::ALT) => {
                if !self.clipboard.is_empty() {
                    // Clone the clipboard content to avoid borrowing issues
                    let clipboard_content = self.clipboard.clone();
                    let ends_with_newline = clipboard_content.ends_with('\n');
                    
                    let tab = self.current_tab_mut();
                    let cursor_y = tab.cursor.y;
                    let cursor_x = tab.cursor.x;
                    
                    // Check if clipboard ends with newline to determine paste style
                    if ends_with_newline {
                        // Paste on new line above current line
                        // First, move to the beginning of the current line
                        tab.cursor.x = 0;
                        
                        // Calculate clipboard lines
                        let clipboard_lines: Vec<&str> = clipboard_content.lines().collect();
                        
                        // Insert each line from the clipboard
                        for (i, line) in clipboard_lines.iter().enumerate() {
                            // Insert the line content
                            for c in line.chars() {
                                tab.buffer.insert_char_at_cursor(c, &tab.cursor);
                                tab.cursor.x += 1;
                            }
                            
                            // If not the last line, add a newline
                            if i < clipboard_lines.len() - 1 {
                                tab.buffer.insert_newline_at_cursor(&tab.cursor);
                                tab.cursor.y += 1;
                                tab.cursor.x = 0;
                            }
                        }
                        
                        // Position cursor at the start of the first pasted line
                        tab.cursor.y = cursor_y;
                        tab.cursor.x = 0;
                    } else {
                        // Paste inline before cursor
                        // First, move cursor left (if possible)
                        if cursor_x > 0 {
                            tab.cursor.x -= 1;
                        }
                        
                        // Paste the content
                        for c in clipboard_content.chars() {
                            tab.buffer.insert_char_at_cursor(c, &tab.cursor);
                            tab.cursor.x += 1;
                        }
                        
                        // Move cursor back to original position
                        if cursor_x > 0 {
                            tab.cursor.x = cursor_x;
                        }
                    }
                    
                    self.update_viewport();
                    self.invalidate_highlight_cache();
                }
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
            // Ctrl+n for new tab
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.add_tab();
            },
            // Ctrl+Right for next tab
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.next_tab();
            },
            // Ctrl+Left for previous tab
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.prev_tab();
            },
            // Ctrl+w to close tab
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.close_tab();
            },
            // F-key direct tab access
            KeyCode::F(1) => { self.go_to_tab(0); },
            KeyCode::F(2) => { self.go_to_tab(1); },
            KeyCode::F(3) => { self.go_to_tab(2); },
            KeyCode::F(4) => { self.go_to_tab(3); },
            KeyCode::F(5) => { self.go_to_tab(4); },
            KeyCode::F(6) => { self.go_to_tab(5); },
            KeyCode::F(7) => { self.go_to_tab(6); },
            KeyCode::F(8) => { self.go_to_tab(7); },
            KeyCode::F(9) => { self.go_to_tab(8); },
            KeyCode::F(10) => { self.go_to_tab(9); },
            KeyCode::F(11) => { self.go_to_tab(10); },
            KeyCode::F(12) => { self.go_to_tab(11); },
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
                // Invalidate syntax highlighting cache for the modified line
                self.invalidate_highlight_cache();
            }
            KeyCode::Backspace => {
                let tab = self.current_tab_mut();
                if tab.cursor.x > 0 {
                    // Regular backspace - delete character before cursor
                    tab.cursor.move_left(&tab.buffer);
                    tab.buffer.delete_char_at_cursor(&tab.cursor);
                    self.update_viewport();
                    // Invalidate syntax highlighting cache for the modified line
                    self.invalidate_highlight_cache();
                } else if tab.cursor.y > 0 {
                    // Cursor is at the beginning of a line
                    // Remember the current line's content
                    let current_line_content = tab.buffer.get_line(tab.cursor.y).to_string();
                    
                    // Move cursor to end of previous line
                    let prev_line_len = tab.buffer.line_length(tab.cursor.y - 1);
                    tab.cursor.y -= 1;
                    tab.cursor.x = prev_line_len;
                    
                    // Join the lines
                    tab.buffer.join_line(tab.cursor.y);
                    
                    // Update viewport for new cursor position
                    self.update_viewport();
                    
                    // Invalidate syntax highlighting cache
                    self.invalidate_highlight_cache();
                }
            }
            KeyCode::Enter => {
                let tab = self.current_tab_mut();
                tab.buffer.insert_newline_at_cursor(&tab.cursor);
                tab.cursor.x = 0;
                tab.cursor.y += 1;
                self.update_viewport();
                // Invalidate syntax highlighting cache for the modified lines
                self.invalidate_highlight_cache();
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
                    "normal_mode" => {
                        self.command_text.clear();
                        self.mode = Mode::Normal;
                    },
                    _ => {}
                }
                return Ok(true);
            }
        }

        // Default handling
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.command_text.clear();
                self.mode = Mode::Normal;
            },
            KeyCode::Char(c) => {
                self.command_text.push(c);
            },
            KeyCode::Backspace => {
                self.command_text.pop();
            },
            KeyCode::Enter => {
                // Process the command
                let cmd = self.command_text.clone();
                self.command_text.clear();
                
                // Process the command with shorter commands than Vim
                if cmd == "w" || cmd == "write" {
                    // Write file
                    if let Some(path) = &self.current_tab().buffer.file_path {
                        if path.starts_with("untitled-") {
                            // Need a real filename
                            // TODO: Implement a filename prompt
                            println!("Error: No filename specified. Use :w filename");
                        } else {
                            if let Err(e) = self.current_tab_mut().buffer.save(None) {
                                println!("Error saving file: {}", e);
                            }
                        }
                    } else {
                        println!("Error: No filename specified. Use :w filename");
                    }
                } else if cmd.starts_with("w ") || cmd.starts_with("write ") {
                    // Write to specified file
                    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
                    if parts.len() > 1 {
                        let filename = parts[1].trim();
                        if !filename.is_empty() {
                            if let Err(e) = self.current_tab_mut().buffer.save(Some(filename)) {
                                println!("Error saving file: {}", e);
                            }
                        } else {
                            println!("Error: No filename specified");
                        }
                    }
                } else if cmd == "q" || cmd == "quit" {
                    // Quit
                    return Ok(false);
                } else if cmd == "x" {
                    // Save and quit (shorter than wq)
                    if let Some(path) = &self.current_tab().buffer.file_path {
                        if path.starts_with("untitled-") {
                            // Need a real filename
                            println!("Error: No filename specified. Use :x filename");
                        } else {
                            if let Err(e) = self.current_tab_mut().buffer.save(None) {
                                println!("Error saving file: {}", e);
                            } else {
                                return Ok(false); // Exit
                            }
                        }
                    } else {
                        println!("Error: No filename specified. Use :x filename");
                    }
                } else if cmd.starts_with("x ") {
                    // Write to file and quit (shorter than wq)
                    let filename = cmd[2..].trim();
                    if !filename.is_empty() {
                        if let Err(e) = self.current_tab_mut().buffer.save(Some(filename)) {
                            println!("Error saving file: {}", e);
                        } else {
                            return Ok(false); // Exit
                        }
                    } else {
                        println!("Error: No filename specified");
                    }
                } else if cmd == "q!" || cmd == "quit!" {
                    // Force quit
                    return Ok(false);
                } else if cmd == "e" || cmd == "edit" {
                    // Refresh current file (reload from disk)
                    if let Some(path) = &self.current_tab().buffer.file_path.clone() {
                        if !path.starts_with("untitled-") {
                            if let Err(e) = self.current_tab_mut().buffer.load_file(path) {
                                println!("Error reloading file: {}", e);
                            }
                        }
                    }
                }
                
                // Return to normal mode
                self.mode = Mode::Normal;
            },
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
                            // KEY FIX: Use load_file_in_new_tab to ensure clean state
                            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                // Load in a new tab if Ctrl is pressed
                                self.load_file_in_new_tab(&file_path)?;
                            } else {
                                // Load in current tab, but make sure it's clean first
                                let current_tab = self.current_tab;
                                
                                // Check if current tab is modified or has content
                                let need_new_tab = {
                                    let tab = &self.tabs[current_tab];
                                    tab.buffer.is_modified || 
                                        (!tab.buffer.lines.is_empty() && 
                                         !(tab.buffer.lines.len() == 1 && tab.buffer.lines[0].is_empty()))
                                };
                                
                                if need_new_tab {
                                    // Create new tab for the file
                                    self.load_file_in_new_tab(&file_path)?;
                                } else {
                                    // Use current tab
                                    self.load_file(&file_path)?;
                                }
                            }
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
                    // KEY FIX: Use load_file_in_new_tab to ensure clean state
                    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                        // Load in a new tab if Ctrl is pressed
                        self.load_file_in_new_tab(&file_path)?;
                    } else {
                        // Load in current tab, but make sure it's clean first
                        let current_tab = self.current_tab;
                        
                        // Check if current tab is modified or has content
                        let need_new_tab = {
                            let tab = &self.tabs[current_tab];
                            tab.buffer.is_modified || 
                                (!tab.buffer.lines.is_empty() && 
                                 !(tab.buffer.lines.len() == 1 && tab.buffer.lines[0].is_empty()))
                        };
                        
                        if need_new_tab {
                            // Create new tab for the file
                            self.load_file_in_new_tab(&file_path)?;
                        } else {
                            // Use current tab
                            self.load_file(&file_path)?;
                        }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use tempfile::tempdir;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    
    #[test]
    fn test_tab_navigation() {
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Initially we have one tab
        assert_eq!(editor.tabs.len(), 1);
        assert_eq!(editor.current_tab, 0);
        
        // Add a new tab
        editor.add_tab();
        assert_eq!(editor.tabs.len(), 2);
        assert_eq!(editor.current_tab, 1); // Should be on the new tab
        
        // Add another tab
        editor.add_tab();
        assert_eq!(editor.tabs.len(), 3);
        assert_eq!(editor.current_tab, 2);
        
        // Navigate to next tab (should wrap around)
        editor.next_tab();
        assert_eq!(editor.current_tab, 0);
        
        // Navigate to previous tab (should go to last tab)
        editor.prev_tab();
        assert_eq!(editor.current_tab, 2);
        
        // Try to close a tab
        editor.close_tab();
        assert_eq!(editor.tabs.len(), 2);
        assert_eq!(editor.current_tab, 1); // Index adjusts when closing current tab
        
        // Close another tab
        editor.close_tab();
        assert_eq!(editor.tabs.len(), 1);
        assert_eq!(editor.current_tab, 0);
        
        // Try to close the last tab (should be prevented)
        editor.close_tab();
        assert_eq!(editor.tabs.len(), 1); // Should still have one tab
        assert_eq!(editor.current_tab, 0);
    }
    
    #[test]
    fn test_direct_tab_access() {
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Add a few tabs
        editor.add_tab();
        editor.add_tab();
        editor.add_tab();
        editor.add_tab();
        
        // We should have 5 tabs total (including the initial one)
        assert_eq!(editor.tabs.len(), 5);
        assert_eq!(editor.current_tab, 4); // Last one added
        
        // Set filenames for each tab
        editor.tabs[0].buffer.file_path = Some("file1.rs".to_string());
        editor.tabs[1].buffer.file_path = Some("file2.rs".to_string());
        editor.tabs[2].buffer.file_path = Some("file3.rs".to_string());
        editor.tabs[3].buffer.file_path = Some("file4.rs".to_string());
        editor.tabs[4].buffer.file_path = Some("file5.rs".to_string());
        
        // Test direct access
        editor.go_to_tab(0);
        assert_eq!(editor.current_tab, 0);
        assert_eq!(editor.current_tab().buffer.file_path.as_ref().unwrap(), "file1.rs");
        
        editor.go_to_tab(2);
        assert_eq!(editor.current_tab, 2);
        assert_eq!(editor.current_tab().buffer.file_path.as_ref().unwrap(), "file3.rs");
        
        // Test out of bounds access (should be ignored)
        editor.go_to_tab(10);
        assert_eq!(editor.current_tab, 2); // Should remain unchanged
        
        // Test F-key behavior by simulating keypresses (just call go_to_tab directly)
        editor.go_to_tab(3); // Equivalent to pressing F4
        assert_eq!(editor.current_tab, 3);
        assert_eq!(editor.current_tab().buffer.file_path.as_ref().unwrap(), "file4.rs");
    }
    
    #[test]
    fn test_tab_file_loading() {
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Add a tab and load a file in it
        editor.add_tab();
        assert_eq!(editor.current_tab, 1);
        
        // Load different files in each tab
        let _ = editor.tabs[0].buffer.set_content("File 1 content");
        let _ = editor.tabs[1].buffer.set_content("File 2 content");
        
        // Check we can switch between tabs and content is preserved
        editor.prev_tab();
        assert_eq!(editor.current_tab, 0);
        assert_eq!(editor.current_tab().buffer.get_content(), "File 1 content");
        
        editor.next_tab();
        assert_eq!(editor.current_tab, 1);
        assert_eq!(editor.current_tab().buffer.get_content(), "File 2 content");
    }
    
    #[test]
    fn test_write_confirmation_mode() {
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Simulate editing a buffer by inserting text
        let cursor = &editor.current_tab().cursor.clone();
        editor.current_tab_mut().buffer.insert_char_at_cursor('H', cursor);
        
        // Check that the modified flag is set
        assert!(editor.current_tab().buffer.is_modified);
        assert_eq!(editor.current_tab().buffer.get_modified_lines().len(), 1);
        
        // Enter write confirmation mode via w key
        let w_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);
        let _ = editor.handle_normal_mode(w_key);
        
        // Check that we're in WriteConfirm mode
        assert_eq!(editor.mode, Mode::WriteConfirm);
        
        // Press ESC to cancel
        let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let _ = editor.handle_write_confirm_mode(esc_key);
        
        // Should be back in normal mode
        assert_eq!(editor.mode, Mode::Normal);
        
        // Buffer should still be modified
        assert!(editor.current_tab().buffer.is_modified);
    }
    
    #[test]
    fn test_modified_line_tracking() {
        let mut buffer = Buffer::new();
        let mut cursor = Cursor::new();
        
        // Initially no lines are modified
        assert_eq!(buffer.get_modified_lines().len(), 0);
        
        // Insert characters
        buffer.insert_char_at_cursor('a', &cursor);
        buffer.insert_char_at_cursor('b', &cursor);
        cursor.x = 2;
        buffer.insert_char_at_cursor('c', &cursor);
        
        // Check that we have one modified line
        assert_eq!(buffer.get_modified_lines().len(), 1);
        assert!(buffer.is_line_modified(0));
        
        // Add a new line
        cursor.x = 3;
        buffer.insert_newline_at_cursor(&cursor);
        
        // Check that we now have two modified lines
        assert_eq!(buffer.get_modified_lines().len(), 2);
        assert!(buffer.is_line_modified(0));
        assert!(buffer.is_line_modified(1));
        
        // Delete a character
        cursor.y = 1;
        cursor.x = 0;
        buffer.insert_char_at_cursor('x', &cursor);
        buffer.delete_char_at_cursor(&cursor);
        
        // Still two modified lines (same line modified again)
        assert_eq!(buffer.get_modified_lines().len(), 2);
    }
    
    #[test]
    fn test_filename_prompt_mode() {
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Enter WriteConfirm mode
        editor.mode = Mode::WriteConfirm;
        
        // Simulate pressing Y to confirm
        let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        let _ = editor.handle_write_confirm_mode(y_key);
        
        // Should now be in FilenamePrompt mode since we have an untitled file
        assert_eq!(editor.mode, Mode::FilenamePrompt);
        
        // Type a filename
        let t_key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
        let e_key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        let s_key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let t2_key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
        let dot_key = KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE);
        let t3_key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
        let x_key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let t4_key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
        
        let _ = editor.handle_filename_prompt_mode(t_key);
        let _ = editor.handle_filename_prompt_mode(e_key);
        let _ = editor.handle_filename_prompt_mode(s_key);
        let _ = editor.handle_filename_prompt_mode(t2_key);
        let _ = editor.handle_filename_prompt_mode(dot_key);
        let _ = editor.handle_filename_prompt_mode(t3_key);
        let _ = editor.handle_filename_prompt_mode(x_key);
        let _ = editor.handle_filename_prompt_mode(t4_key);
        
        // Check that the filename is stored
        assert_eq!(editor.filename_prompt_text, "test.txt");
        
        // Press Escape to cancel
        let esc_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let _ = editor.handle_filename_prompt_mode(esc_key);
        
        // Should be back in normal mode
        assert_eq!(editor.mode, Mode::Normal);
        
        // And filename prompt should be cleared
        assert_eq!(editor.filename_prompt_text, "");
    }
    
    #[test]
    fn test_save_file_flow() -> Result<()> {
        // Create a temporary directory for test files
        let dir = tempdir()?;
        let file_path = dir.path().join("test_save.txt");
        let file_path_str = file_path.to_str().unwrap();
        
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Set up initial content
        let cursor = &editor.current_tab().cursor.clone();
        editor.current_tab_mut().buffer.insert_char_at_cursor('T', cursor);
        editor.current_tab_mut().buffer.file_path = Some(file_path_str.to_string());
        
        // Verify it's modified
        assert!(editor.current_tab().buffer.is_modified);
        
        // Enter write confirm mode
        editor.mode = Mode::WriteConfirm;
        
        // Confirm saving
        let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        let _ = editor.handle_write_confirm_mode(y_key);
        
        // Verify the file exists and has the correct content
        let content = fs::read_to_string(&file_path)?;
        assert_eq!(content, "T");
        
        // Verify the buffer is no longer marked as modified
        assert!(!editor.current_tab().buffer.is_modified);
        assert_eq!(editor.current_tab().buffer.get_modified_lines().len(), 0);
        
        Ok(())
    }
    
    #[test]
    fn test_reload_file_flow() -> Result<()> {
        println!("Starting reload file flow test");
        // Create a temporary directory for test files
        let dir = tempdir()?;
        let file_path = dir.path().join("test_reload.txt");
        let file_path_str = file_path.to_str().unwrap();
        
        println!("Creating test file at: {}", file_path_str);
        // Create initial file content on disk
        std::fs::write(&file_path, "Initial content")?;
        
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Load the initial file content
        println!("Loading initial file content");
        editor.current_tab_mut().buffer.file_path = Some(file_path_str.to_string());
        editor.current_tab_mut().buffer.load_file(file_path_str)?;
        
        // Verify the initial content is loaded
        let content = editor.current_tab().buffer.get_content();
        println!("Initial content loaded: '{}'", content);
        assert_eq!(content, "Initial content");
        
        // Make a change to the buffer
        println!("Making change to buffer");
        let mut cursor = Cursor::new();
        cursor.x = 8; // After "Initial "
        editor.current_tab_mut().buffer.insert_char_at_cursor('X', &cursor);
        
        // Verify buffer is modified
        let modified_content = editor.current_tab().buffer.get_content();
        println!("Modified content: '{}'", modified_content);
        assert!(editor.current_tab().buffer.is_modified);
        assert_eq!(modified_content, "Initial Xcontent");
        
        // Change the file on disk
        println!("Changing file on disk");
        std::fs::write(&file_path, "Changed on disk")?;
        
        // Verify file content changed
        let disk_content = std::fs::read_to_string(&file_path)?;
        println!("Disk content: '{}'", disk_content);
        
        // Calculate diff between buffer and disk
        println!("Calculating diff");
        let diff = editor.current_tab().buffer.diff_with_disk()?;
        println!("Diff lines: {:?}", diff);
        assert!(!diff.is_empty());
        
        // Set up reload confirm mode
        println!("Setting up reload confirm mode");
        editor.diff_lines = diff;
        editor.mode = Mode::ReloadConfirm;
        
        // Confirm reload
        println!("Confirming reload");
        let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        let _ = editor.handle_reload_confirm_mode(y_key);
        
        // Verify the buffer was reloaded with disk content
        let final_content = editor.current_tab().buffer.get_content();
        println!("Final content: '{}'", final_content);
        assert_eq!(final_content, "Changed on disk");
        
        // Verify we're back in normal mode
        assert_eq!(editor.mode, Mode::Normal);
        
        // Verify diff lines were cleared
        assert!(editor.diff_lines.is_empty());
        
        println!("Test completed successfully");
        Ok(())
    }
    
    #[test]
    fn test_visual_mode_basics() {
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // The editor now starts in FileFinder mode by default
        assert_eq!(editor.mode, Mode::FileFinder);
        
        // Switch to normal mode for the test
        editor.mode = Mode::Normal;
        
        // Check that the buffer starts with no selection
        assert_eq!(editor.current_tab().buffer.selection_start, None);
        
        // Enter visual mode - this will initialize the selection
        editor.mode = Mode::Visual;
        let current_pos = (editor.current_tab().cursor.y, editor.current_tab().cursor.x);
        editor.current_tab_mut().buffer.start_selection(current_pos);
        
        // Check that we're in visual mode and have a selection
        assert_eq!(editor.mode, Mode::Visual);
        assert!(editor.current_tab().buffer.selection_start.is_some());
        
        // Exit visual mode
        editor.mode = Mode::Normal;
        editor.current_tab_mut().buffer.clear_selection();
        
        // Check that we're back in normal mode with no selection
        assert_eq!(editor.mode, Mode::Normal);
        assert_eq!(editor.current_tab().buffer.selection_start, None);
    }
}