mod buffer;
mod cursor;
mod mode;
mod file_finder;
mod viewport;
mod diagnostics;
mod syntax;
mod snake;
mod history;

pub use buffer::Buffer;
pub use cursor::Cursor;
pub use mode::Mode;
pub use file_finder::FileFinder;
pub use viewport::Viewport;
pub use diagnostics::{DiagnosticSeverity, DiagnosticCollection};
pub use syntax::{SyntaxHighlighter, HighlightedLine};
pub use snake::{Snake, Direction, GameState, Position};

use anyhow::Result;
use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
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
    EnterTokenSearch,
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
    
    // Editing operations
    Undo,
    Redo,
    
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

/// Filter for the diagnostics panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticFilter {
    All,
    Errors,
    Warnings,
    Info,
}

impl Default for DiagnosticFilter {
    fn default() -> Self {
        Self::All
    }
}

pub struct Editor {
    pub tabs: Vec<Tab>,
    pub current_tab: usize,
    pub mode: Mode,
    pub file_finder: FileFinder,
    pub token_search: TokenSearch,
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
    /// Selected diagnostic index for the diagnostics panel
    pub selected_diagnostic_index: usize,
    /// Current filter for the diagnostics panel
    pub diagnostics_filter: DiagnosticFilter,
    /// Snake game instance (Easter egg)
    pub snake_game: Option<Snake>,
}

use grep::matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;
use ignore::Walk;
use anyhow::Context;
use regex;

/// Structure for token search functionality
pub struct TokenSearch {
    pub query: String,
    pub results: Vec<TokenSearchResult>,
    pub selected_index: usize,
}

/// Represents a token search result
#[derive(Clone)]
pub struct TokenSearchResult {
    pub file_path: String,
    pub line_number: usize,
    pub column: usize,
    pub line_content: String,
    pub matched_text: String,
}

impl TokenSearch {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected_index: 0,
        }
    }
    
    /// Add a character to the search query
    pub fn add_char(&mut self, c: char) {
        self.query.push(c);
    }
    
    /// Remove the last character from the search query
    pub fn remove_char(&mut self) {
        self.query.pop();
    }
    
    /// Move to the next search result
    pub fn next(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }
    
    /// Move to the previous search result
    pub fn previous(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.results.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }
    
    /// Get the currently selected result
    pub fn get_selected(&self) -> Option<&TokenSearchResult> {
        self.results.get(self.selected_index)
    }
    
    /// Get a clone of the currently selected result
    pub fn get_selected_cloned(&self) -> Option<TokenSearchResult> {
        self.results.get(self.selected_index).cloned()
    }
    
    /// Perform a search for the current query across all project files using ripgrep
    pub fn search(&mut self) -> Result<()> {
        self.results.clear();
        self.selected_index = 0;
        
        // If query is empty, return early
        if self.query.is_empty() {
            return Ok(());
        }
        
        // Get current directory
        let current_dir = std::env::current_dir()
            .context("Failed to get current directory")?;
        
        // Use ignore crate to respect .gitignore files and other common ignore patterns
        let mut results = Vec::new();
        
        // Create a matcher with case-insensitive search
        // Escape the query to treat it as a literal string for fuzzy matches
        let regex_query = regex::escape(&self.query);
        
        // Make the regex case-insensitive by prefixing with (?i)
        let case_insensitive_query = format!("(?i){}", regex_query);
        
        let matcher = match RegexMatcher::new(&case_insensitive_query) {
            Ok(m) => m,
            Err(e) => {
                // Return a user-friendly error if the regex is invalid
                return Err(anyhow::anyhow!("Invalid search pattern: {}", e));
            }
        };
        
        // Create a searcher
        let mut searcher = Searcher::new();
        
        // Configure the searcher for multi-line results
        searcher.multi_line_with_matcher(&matcher);
        
        // Walk through all files in current directory, respecting .gitignore
        for result in Walk::new(&current_dir) {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue, // Skip entries with errors
            };
            
            // Skip directories
            if entry.path().is_dir() {
                continue;
            }
            
            let path = entry.path();
            
            // Get relative path for display
            let file_path = match path.strip_prefix(&current_dir) {
                Ok(rel_path) => rel_path.to_string_lossy().to_string(),
                Err(_) => path.to_string_lossy().to_string(),
            };
            
            // Search for matches in the file
            let _ = searcher.search_path(&matcher, path, UTF8(|line_number, line| {
                // Find match column position
                if let Some(grep_match) = matcher.find(line.as_bytes())? {
                    let start = grep_match.start();
                    let col = start;
                    
                    // Extract matched text
                    let end = std::cmp::min(start + self.query.len(), line.len());
                    let matched_text = &line[start..end];
                    
                    // Create result entry
                    let result = TokenSearchResult {
                        file_path: file_path.clone(),
                        line_number: line_number as usize,
                        column: col,
                        line_content: line.trim_end().to_string(),
                        matched_text: matched_text.to_string(),
                    };
                    
                    results.push(result);
                }
                
                // Always return Ok to continue searching
                Ok(true)
            }));
            
            // Limit results for performance (check periodically)
            if results.len() > 1000 {
                break;
            }
        }
        
        // Store results
        self.results = results;
        
        Ok(())
    }
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
            token_search: TokenSearch::new(),
            config,
            save_and_quit: false,
            command_text: String::new(),
            filename_prompt_text: String::new(),
            diff_lines: HashSet::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            highlighted_lines_cache: HashMap::new(),
            clipboard: String::new(),
            selected_diagnostic_index: 0,
            diagnostics_filter: DiagnosticFilter::default(),
            snake_game: None,
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
/// Find the project root directory by looking for Cargo.toml
pub fn find_project_root(&self) -> Option<String> {
    // First try from the current buffer's file path
    if let Some(file_path) = &self.current_tab().buffer.file_path {
        let path = std::path::Path::new(file_path);
        let mut dir = path.parent();
        
        // Walk up the directory tree looking for Cargo.toml
        while let Some(current_dir) = dir {
            let cargo_toml = current_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                return Some(current_dir.to_string_lossy().to_string());
            }
            dir = current_dir.parent();
        }
    }
    
    // Fallback to current directory
    match std::env::current_dir() {
        Ok(path) => Some(path.to_string_lossy().to_string()),
        Err(_) => None
    }
}

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
    
    // Combine stdout and stderr
    let full_output = format!("{}\n{}", stdout, stderr);
    
    // Parse the diagnostics, scoping to the current file
    {
        let tab = self.current_tab_mut();
        let diagnostics = std::mem::take(&mut tab.diagnostics);
        tab.diagnostics = diagnostics.parse_cargo_output(&full_output, &current_file);
    }
    
    // Check for diagnostics and process in smaller scopes to avoid borrow issues
    let has_diagnostics;
    let first_line_opt;
    
    // First, gather information about diagnostics
    {
        let tab = self.current_tab();
        has_diagnostics = tab.diagnostics.error_count() > 0 || tab.diagnostics.warning_count() > 0;
        first_line_opt = if has_diagnostics {
            tab.diagnostics.get_diagnostic_line_numbers().first().copied()
        } else {
            None
        };
    }
    
    // Then, set the mode based on diagnostics
    if has_diagnostics {
        self.mode = Mode::DiagnosticsPanel;
        self.selected_diagnostic_index = 0; // Reset to first diagnostic
    }
    
    // Finally, navigate to the diagnostic if available
    if let Some(first_line) = first_line_opt {
        let first_column;
        
        // Get the column in a separate scope
        {
            let tab = self.current_tab();
            if let Some(diagnostics) = tab.diagnostics.get_diagnostics_for_line(first_line) {
                if !diagnostics.is_empty() {
                    first_column = diagnostics[0].span.start_column;
                } else {
                    first_column = 0;
                }
            } else {
                first_column = 0;
            }
        }
        
        // Update cursor and viewport
        {
            let tab = self.current_tab_mut();
            tab.cursor.y = first_line;
            tab.cursor.x = first_column;
            
            // Position the line with better context (not at the top edge)
            let desired_offset = tab.viewport.height / 3;
            if first_line > desired_offset {
                tab.viewport.top_line = first_line.saturating_sub(desired_offset);
            } else {
                tab.viewport.top_line = 0;
            }
        }
        
        // Ensure the cursor is visible
        self.update_viewport();
    }
    
    Ok(())
}

/// Navigate to the next diagnostic in the current file
pub fn goto_next_diagnostic(&mut self) -> Result<()> {
    let tab = self.current_tab();
    if let Some(next_line) = tab.diagnostics.next_diagnostic_line(tab.cursor.y) {
        // Position cursor at the next diagnostic line
        let tab = self.current_tab_mut();
        tab.cursor.y = next_line;
        
        // If there's a diagnostic for this line, position at its column
        if let Some(diagnostics) = tab.diagnostics.get_diagnostics_for_line(next_line) {
            if !diagnostics.is_empty() {
                tab.cursor.x = diagnostics[0].span.start_column;
            }
        }
        
        // Position the line with better context (not at the top edge)
        let desired_offset = tab.viewport.height / 3;
        if next_line > desired_offset {
            tab.viewport.top_line = next_line.saturating_sub(desired_offset);
        } else {
            tab.viewport.top_line = 0;
        }
        
        // Ensure the cursor is visible
        self.update_viewport();
    }
    
    Ok(())
}

/// Navigate to the previous diagnostic in the current file
pub fn goto_prev_diagnostic(&mut self) -> Result<()> {
    let tab = self.current_tab();
    if let Some(prev_line) = tab.diagnostics.prev_diagnostic_line(tab.cursor.y) {
        // Position cursor at the previous diagnostic line
        let tab = self.current_tab_mut();
        tab.cursor.y = prev_line;
        
        // If there's a diagnostic for this line, position at its column
        if let Some(diagnostics) = tab.diagnostics.get_diagnostics_for_line(prev_line) {
            if !diagnostics.is_empty() {
                tab.cursor.x = diagnostics[0].span.start_column;
            }
        }
        
        // Position the line with better context (not at the top edge)
        let desired_offset = tab.viewport.height / 3;
        if prev_line > desired_offset {
            tab.viewport.top_line = prev_line.saturating_sub(desired_offset);
        } else {
            tab.viewport.top_line = 0;
        }
        
        // Ensure the cursor is visible
        self.update_viewport();
    }
    
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
            
            // Run diagnostics in the background for the newly loaded file
            if let Some(project_dir) = self.find_project_root() {
                if let Err(_) = self.run_cargo_command(&project_dir, "check") {
                    // Silently ignore errors in background diagnostics
                }
            }
        }
        
        result
    }
    
    /// Load file in a new tab
    pub fn load_file_in_new_tab(&mut self, path: &str) -> Result<()> {
        // Check if a tab already exists with this file
        if let Some(tab_index) = self.tabs.iter().position(|tab| 
            tab.buffer.file_path.as_ref().map_or(false, |f| f == path)
        ) {
            // If tab exists, switch to it
            self.current_tab = tab_index;
            Ok(())
        } else {
            // Create a new tab and load the file
            self.add_tab();
            self.load_file(path)
        }
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
            Mode::TokenSearch => self.handle_token_search_mode(key),
            Mode::DiagnosticsPanel => self.handle_diagnostics_panel_mode(key),
            Mode::Snake => self.handle_snake_mode(key),
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
            // Delete mode with composable delete operations
            Mode::Delete => self.handle_delete_mode(key),
        }
    }
    
    /// Handle mouse events in the editor
    ///
    /// This function handles mouse events, particularly scroll events,
    /// to allow users to scroll the editor with the mouse wheel.
    pub fn handle_mouse(&mut self, mouse_event: MouseEvent) -> Result<bool> {
        // Check if we have any tabs and the current tab index is valid
        if self.tabs.is_empty() || self.current_tab >= self.tabs.len() {
            return Ok(true); // Do nothing if no valid tabs
        }
        
        match mouse_event.kind {
            MouseEventKind::ScrollDown => {
                // Scroll the viewport down 3 lines
                let tab = self.current_tab_mut();
                let max_line = tab.buffer.lines.len();
                tab.viewport.scroll_down(3, max_line);
                Ok(true)
            },
            MouseEventKind::ScrollUp => {
                // Scroll the viewport up 3 lines
                let tab = self.current_tab_mut();
                tab.viewport.scroll_up(3);
                Ok(true)
            },
            _ => Ok(true), // Ignore other mouse events for now
        }
    }
    
    /// Handle key events in delete mode
    fn handle_delete_mode(&mut self, key: KeyEvent) -> Result<bool> {
        use crossterm::event::{KeyCode, KeyModifiers};
        
        match key.code {
            KeyCode::Esc => {
                // Cancel delete operation
                self.mode = Mode::Normal;
                Ok(true)
            },
            KeyCode::Char('d') => {
                // Delete current line (dd)
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
                self.mode = Mode::Normal;
                Ok(true)
            },
            KeyCode::Char('w') => {
                // Delete word
                let tab = self.current_tab_mut();
                tab.buffer.delete_word_at_cursor(&mut tab.cursor);
                self.update_viewport();
                self.invalidate_highlight_cache();
                self.mode = Mode::Normal;
                Ok(true)
            },
            KeyCode::Char('$') => {
                // Delete to end of line
                let tab = self.current_tab_mut();
                tab.buffer.delete_to_end_of_line(&tab.cursor);
                self.update_viewport();
                self.invalidate_highlight_cache();
                self.mode = Mode::Normal;
                Ok(true)
            },
            KeyCode::Char('^') | KeyCode::Char('0') => {
                // Delete to beginning of line
                let tab = self.current_tab_mut();
                tab.buffer.delete_to_beginning_of_line(&tab.cursor);
                self.update_viewport();
                self.invalidate_highlight_cache();
                self.mode = Mode::Normal;
                Ok(true)
            },
            // Any other key cancels delete operation
            _ => {
                self.mode = Mode::Normal;
                self.handle_normal_mode(key)
            }
        }
    }
    
    fn handle_snake_mode(&mut self, key: KeyEvent) -> Result<bool> {
        use crossterm::event::{KeyCode, KeyModifiers};
        
        if let Some(snake) = &mut self.snake_game {
            match key.code {
                KeyCode::Esc => {
                    // Exit snake game
                    self.snake_game = None;
                    self.mode = Mode::Normal;
                },
                KeyCode::Char('q') => {
                    // Exit snake game
                    self.snake_game = None;
                    self.mode = Mode::Normal;
                },
                KeyCode::Char('r') => {
                    // Reset game
                    snake.reset();
                },
                KeyCode::Up | KeyCode::Char('k') => {
                    snake.change_direction(Direction::Up);
                },
                KeyCode::Down | KeyCode::Char('j') => {
                    snake.change_direction(Direction::Down);
                },
                KeyCode::Left | KeyCode::Char('h') => {
                    snake.change_direction(Direction::Left);
                },
                KeyCode::Right | KeyCode::Char('l') => {
                    snake.change_direction(Direction::Right);
                },
                _ => {}
            }
        }
        
        Ok(true)
    }
    
    /// Start the snake game
    pub fn start_snake_game(&mut self) {
        // Get viewport dimensions for game area
        let tab = self.current_tab();
        
        // Make an even smaller game area for better gameplay
        // Target about 20x15 cells for a good game experience
        let width = std::cmp::min(20, tab.viewport.width / 4);
        let height = std::cmp::min(15, tab.viewport.height / 3);
        
        // Create a new snake game
        self.snake_game = Some(Snake::new(width, height));
        self.mode = Mode::Snake;
    }
    
    /// Handle key events in token search mode
    /// Handler for the diagnostics panel mode
    fn handle_diagnostics_panel_mode(&mut self, key: KeyEvent) -> Result<bool> {
        use crossterm::event::{KeyCode, KeyModifiers};
        
        match key.code {
            KeyCode::Esc => {
                // Return to normal mode
                self.mode = Mode::Normal;
            },
            KeyCode::Char('q') => {
                // Return to normal mode
                self.mode = Mode::Normal;
            },
            // Handle filter switching keys
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // Switch to All filter
                self.diagnostics_filter = DiagnosticFilter::All;
                self.selected_diagnostic_index = 0; // Reset selection
            },
            KeyCode::Char('e') | KeyCode::Char('E') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Switch to Errors filter
                self.diagnostics_filter = DiagnosticFilter::Errors;
                self.selected_diagnostic_index = 0; // Reset selection
            },
            KeyCode::Char('w') | KeyCode::Char('W') => {
                // Switch to Warnings filter
                self.diagnostics_filter = DiagnosticFilter::Warnings;
                self.selected_diagnostic_index = 0; // Reset selection
            },
            KeyCode::Char('i') | KeyCode::Char('I') => {
                // Switch to Info filter
                self.diagnostics_filter = DiagnosticFilter::Info;
                self.selected_diagnostic_index = 0; // Reset selection
            },
            // Navigation keys
            KeyCode::Char('n') | KeyCode::Down | KeyCode::Char('j') => {
                // Move to next diagnostic in the panel
                let diagnostics = self.current_tab().diagnostics.get_filtered_diagnostics(&self.diagnostics_filter);
                if !diagnostics.is_empty() {
                    self.selected_diagnostic_index = (self.selected_diagnostic_index + 1) % diagnostics.len();
                }
            },
            KeyCode::Char('p') | KeyCode::Up | KeyCode::Char('k') => {
                // Move to previous diagnostic in the panel
                let diagnostics = self.current_tab().diagnostics.get_filtered_diagnostics(&self.diagnostics_filter);
                if !diagnostics.is_empty() {
                    self.selected_diagnostic_index = if self.selected_diagnostic_index == 0 {
                        diagnostics.len() - 1
                    } else {
                        self.selected_diagnostic_index - 1
                    };
                }
            },
            KeyCode::Enter => {
                // Navigate to the selected diagnostic and switch back to normal mode
                let diagnostics = self.current_tab().diagnostics.get_filtered_diagnostics(&self.diagnostics_filter);
                
                if !diagnostics.is_empty() && self.selected_diagnostic_index < diagnostics.len() {
                    // Get the selected diagnostic - clone the necessary fields to avoid borrow conflicts
                    let line = diagnostics[self.selected_diagnostic_index].span.line;
                    let start_column = diagnostics[self.selected_diagnostic_index].span.start_column;
                    
                    // Position cursor at the diagnostic location
                    let tab = self.current_tab_mut();
                    tab.cursor.y = line;
                    tab.cursor.x = start_column;
                    
                    // Position the line with better context (not at the top edge)
                    let desired_offset = tab.viewport.height / 3;
                    if line > desired_offset {
                        tab.viewport.top_line = line.saturating_sub(desired_offset);
                    } else {
                        tab.viewport.top_line = 0;
                    }
                    
                    // Ensure the cursor is visible
                    self.update_viewport();
                    
                    // Switch back to normal mode
                    self.mode = Mode::Normal;
                }
            },
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Return to normal mode on Ctrl+E (toggle)
                self.mode = Mode::Normal;
            },
            _ => {
                // Pass keys like h, l, $ etc. to normal mode handler
                return self.handle_normal_mode(key);
            }
        }
        
        Ok(true)
    }
    
    fn handle_token_search_mode(&mut self, key: KeyEvent) -> Result<bool> {
        let bindings = &self.config.key_bindings.token_search_mode;
        
        // Check bindings first
        for (command, binding) in bindings {
            if binding.matches(&key) {
                match command.as_str() {
                    "cancel" => {
                        // Exit token search mode
                        self.mode = Mode::Normal;
                        return Ok(true);
                    },
                    "select" => {
                        // Navigate to the selected search result
                        if let Some(result) = self.token_search.get_selected_cloned() {
                            // Check if we need to load a different file
                            let current_file = self.current_tab().buffer.file_path.clone();
                            
                            if current_file.as_ref().map(|p| p != &result.file_path).unwrap_or(true) {
                                // Load the file that contains the match
                                self.load_file_in_new_tab(&result.file_path)?;
                            }
                            
                            // Position cursor at the match location
                            let tab = self.current_tab_mut();
                            tab.cursor.y = result.line_number;
                            tab.cursor.x = result.column;
                            
                            // Position the line with better context (not at the top edge)
                            // Try to position the line at 1/3 of the viewport height from the top
                            let desired_offset = tab.viewport.height / 3;
                            if result.line_number > desired_offset {
                                tab.viewport.top_line = result.line_number - desired_offset;
                            } else {
                                tab.viewport.top_line = 0;
                            }
                            
                            // Ensure the matched line is visible
                            self.update_viewport();
                            
                            // Switch back to normal mode
                            self.mode = Mode::Normal;
                        }
                        return Ok(true);
                    },
                    "next" => {
                        self.token_search.next();
                        return Ok(true);
                    },
                    "previous" => {
                        self.token_search.previous();
                        return Ok(true);
                    },
                    _ => {}
                }
            }
        }
        
        // Default handling for keys not bound in key_bindings
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                // Exit token search mode
                self.mode = Mode::Normal;
            },
            KeyCode::Enter => {
                // Navigate to the selected search result
                if let Some(result) = self.token_search.get_selected_cloned() {
                    // Check if we need to load a different file
                    let current_file = self.current_tab().buffer.file_path.clone();
                    
                    if current_file.as_ref().map(|p| p != &result.file_path).unwrap_or(true) {
                        // Load the file that contains the match
                        self.load_file_in_new_tab(&result.file_path)?;
                    }
                    
                    // Position cursor at the match location
                    let tab = self.current_tab_mut();
                    tab.cursor.y = result.line_number;
                    tab.cursor.x = result.column;
                    
                    // Position the line with better context (not at the top edge)
                    // Try to position the line at 1/3 of the viewport height from the top
                    let desired_offset = tab.viewport.height / 3;
                    if result.line_number > desired_offset {
                        tab.viewport.top_line = result.line_number - desired_offset;
                    } else {
                        tab.viewport.top_line = 0;
                    }
                    
                    // Ensure the matched line is visible
                    self.update_viewport();
                    
                    // Switch back to normal mode
                    self.mode = Mode::Normal;
                }
            },
            KeyCode::Char(c) => {
                // Add character to search
                self.token_search.add_char(c);
                
                // Perform the search with the updated query
                // Use a small delay for better UX if typing quickly
                if self.token_search.query.len() > 2 {
                    let _ = self.token_search.search();
                }
            },
            KeyCode::Backspace => {
                // Remove character from search
                self.token_search.remove_char();
                
                // Update search results if query is not empty
                if !self.token_search.query.is_empty() && self.token_search.query.len() > 2 {
                    let _ = self.token_search.search();
                }
            },
            KeyCode::Down => {
                self.token_search.next();
            },
            KeyCode::Up => {
                self.token_search.previous();
            },
            _ => {}
        }
        
        Ok(true)
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
                            // Error will be displayed in UI status line
                        } else {
                            // Add to recent files list
                            self.file_finder.add_recent_file(&path);
                            
                            // Run diagnostics in the background after saving
                            if let Some(project_dir) = self.find_project_root() {
                                if let Err(_) = self.run_cargo_command(&project_dir, "check") {
                                    // Silently ignore errors in background diagnostics
                                }
                            }
                            
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
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // User wants to save all tabs (equivalent to :wa in vim)
                let mut all_saved = true;
                
                for tab_idx in 0..self.tabs.len() {
                    // Switch to the tab temporarily
                    let original_tab = self.current_tab;
                    self.current_tab = tab_idx;
                    
                    // Try to save the tab
                    if let Some(path) = self.current_tab().buffer.file_path.clone() {
                        if !path.starts_with("untitled-") {
                            if let Err(_) = self.current_tab_mut().buffer.save(None) {
                                // Error saving this tab
                                all_saved = false;
                            } else {
                                // Add to recent files list
                                self.file_finder.add_recent_file(&path);
                                
                                // Run diagnostics in the background after saving
                                if let Some(project_dir) = self.find_project_root() {
                                    if let Err(_) = self.run_cargo_command(&project_dir, "check") {
                                        // Silently ignore errors in background diagnostics
                                    }
                                }
                            }
                        } else {
                            // Untitled files need a real name, so we can't auto-save them
                            all_saved = false;
                        }
                    } else {
                        // No filename, can't save
                        all_saved = false;
                    }
                    
                    // Switch back to the original tab
                    self.current_tab = original_tab;
                }
                
                // Check if we should quit after saving all
                if should_quit && all_saved {
                    self.save_and_quit = false;
                    return Ok(false); // Exit the editor
                }
                
                // Return to normal mode
                self.mode = Mode::Normal;
                Ok(true)
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
                    if let Err(_) = self.current_tab_mut().buffer.save(Some(&filename)) {
                        // Error will be displayed in status bar
                    } else {
                        // Add to recent files
                        self.file_finder.add_recent_file(&filename);
                        
                        // Run diagnostics in the background after saving with new filename
                        if let Some(project_dir) = self.find_project_root() {
                            if let Err(_) = self.run_cargo_command(&project_dir, "check") {
                                // Silently ignore errors in background diagnostics
                            }
                        }
                        
                        // Check if we should quit after saving
                        if should_quit {
                            self.save_and_quit = false;
                            return Ok(false); // Exit the editor
                        }
                    }
                } else {
                    // Error will be displayed in status bar
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
        // Directly handle 'd' key to enter delete mode before checking bindings
        if let KeyCode::Char('d') = key.code {
            if !key.modifiers.contains(KeyModifiers::CONTROL) && 
               !key.modifiers.contains(KeyModifiers::ALT) && 
               !key.modifiers.contains(KeyModifiers::SHIFT) {
                // Explicitly set the mode to Delete
                self.mode = Mode::Delete;
                return Ok(true);
            }
        }
    
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
                    "snake_game" => {
                        // Easter egg: Start snake game
                        self.start_snake_game();
                    },
                    "reload_file" => {
                        // Shortcut for reloading file (directly from normal mode)
                        if let Some(path) = &self.current_tab().buffer.file_path.clone() {
                            if !path.starts_with("untitled-") {
                                if let Err(_) = self.current_tab_mut().buffer.load_file(path) {
                                    // Error will be displayed in status bar
                                } else {
                                    // Run diagnostics in the background for the reloaded file
                                    if let Some(project_dir) = self.find_project_root() {
                                        if let Err(_) = self.run_cargo_command(&project_dir, "check") {
                                            // Silently ignore errors in background diagnostics
                                        }
                                    }
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
                    "token_search" => {
                        // Enter token search mode
                        self.mode = Mode::TokenSearch;
                        self.token_search = TokenSearch::new();
                    },
                    "delete_line" => {
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
                    "undo" => {
                        let tab = self.current_tab_mut();
                        if tab.buffer.undo(&mut tab.cursor) {
                            self.update_viewport();
                            self.invalidate_highlight_cache();
                        }
                    },
                    "redo" => {
                        let tab = self.current_tab_mut();
                        if tab.buffer.redo(&mut tab.cursor) {
                            self.update_viewport();
                            self.invalidate_highlight_cache();
                        }
                    },
                    "open_line_below" => {
                        let cursor_y = self.current_tab().cursor.y;
                        let new_line_idx = self.current_tab_mut().buffer.open_line_below(cursor_y);
                        
                        // Move cursor to the new line
                        let tab = self.current_tab_mut();
                        tab.cursor.y = new_line_idx;
                        tab.cursor.x = 0;
                        
                        // Switch to insert mode
                        self.mode = Mode::Insert;
                        
                        // Update viewport and invalidate highlighting
                        self.update_viewport();
                        self.invalidate_highlight_cache();
                    },
                    "open_line_above" => {
                        let cursor_y = self.current_tab().cursor.y;
                        let new_line_idx = self.current_tab_mut().buffer.open_line_above(cursor_y);
                        
                        // Move cursor to the new line
                        let tab = self.current_tab_mut();
                        tab.cursor.y = new_line_idx;
                        tab.cursor.x = 0;
                        
                        // Switch to insert mode
                        self.mode = Mode::Insert;
                        
                        // Update viewport and invalidate highlighting
                        self.update_viewport();
                        self.invalidate_highlight_cache();
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
            KeyCode::Char('s') => self.start_snake_game(),
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
            // Add handling for d (enter delete mode)
            KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Enter delete mode instead of immediately deleting the line
                self.mode = Mode::Delete;
            },
            // o to open line below current line
            KeyCode::Char('o') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::SHIFT) => {
                let cursor_y = self.current_tab().cursor.y;
                let new_line_idx = self.current_tab_mut().buffer.open_line_below(cursor_y);
                
                // Move cursor to the new line and switch to insert mode
                let tab = self.current_tab_mut();
                tab.cursor.y = new_line_idx;
                tab.cursor.x = 0;
                self.mode = Mode::Insert;
                
                self.update_viewport();
                self.invalidate_highlight_cache();
            },
            // O to open line above current line
            KeyCode::Char('O') if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                let cursor_y = self.current_tab().cursor.y;
                let new_line_idx = self.current_tab_mut().buffer.open_line_above(cursor_y);
                
                // Move cursor to the new line and switch to insert mode
                let tab = self.current_tab_mut();
                tab.cursor.y = new_line_idx;
                tab.cursor.x = 0;
                self.mode = Mode::Insert;
                
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
            // Token search with Ctrl+T
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = Mode::TokenSearch;
                self.token_search = TokenSearch::new();
            },
            // Toggle diagnostics panel with Ctrl+E (errors)
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.mode == Mode::DiagnosticsPanel {
                    self.mode = Mode::Normal;
                } else {
                    self.mode = Mode::DiagnosticsPanel;
                    
                    // If there are diagnostics, navigate to the first one
                    let _ = self.goto_next_diagnostic();
                }
            },
            // Navigate to next diagnostic with Ctrl+N (next error)
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) && key.modifiers.contains(KeyModifiers::SHIFT) => {
                let _ = self.goto_next_diagnostic();
            },
            // Navigate to previous diagnostic with Ctrl+P (previous error)
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) && key.modifiers.contains(KeyModifiers::SHIFT) => {
                let _ = self.goto_prev_diagnostic();
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
            // Undo (u)
            KeyCode::Char('u') => {
                let tab = self.current_tab_mut();
                if tab.buffer.undo(&mut tab.cursor) {
                    self.update_viewport();
                    self.invalidate_highlight_cache();
                }
            },
            // Redo (Ctrl+r)
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let tab = self.current_tab_mut();
                if tab.buffer.redo(&mut tab.cursor) {
                    self.update_viewport();
                    self.invalidate_highlight_cache();
                }
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
                            // Error will be displayed in status bar
                        } else {
                            if let Err(_) = self.current_tab_mut().buffer.save(None) {
                                // Error will be displayed in status bar
                            }
                        }
                    } else {
                        // Error will be displayed in status bar
                    }
                } else if cmd.starts_with("w ") || cmd.starts_with("write ") {
                    // Write to specified file
                    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
                    if parts.len() > 1 {
                        let filename = parts[1].trim();
                        if !filename.is_empty() {
                            if let Err(_) = self.current_tab_mut().buffer.save(Some(filename)) {
                                // Error will be displayed in status bar
                            }
                        } else {
                            // Error will be displayed in status bar
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
                            // Error will be displayed in status bar
                        } else {
                            if let Err(_) = self.current_tab_mut().buffer.save(None) {
                                // Error will be displayed in status bar
                            } else {
                                return Ok(false); // Exit
                            }
                        }
                    } else {
                        // Error will be displayed in status bar
                    }
                } else if cmd.starts_with("x ") {
                    // Write to file and quit (shorter than wq)
                    let filename = cmd[2..].trim();
                    if !filename.is_empty() {
                        if let Err(_) = self.current_tab_mut().buffer.save(Some(filename)) {
                            // Error will be displayed in status bar
                        } else {
                            return Ok(false); // Exit
                        }
                    } else {
                        // Error will be displayed in status bar
                    }
                } else if cmd == "q!" || cmd == "quit!" {
                    // Force quit
                    return Ok(false);
                } else if cmd == "e" || cmd == "edit" {
                    // Refresh current file (reload from disk)
                    if let Some(path) = &self.current_tab().buffer.file_path.clone() {
                        if !path.starts_with("untitled-") {
                            if let Err(_) = self.current_tab_mut().buffer.load_file(path) {
                                // Error will be displayed in status bar
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
                            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                // Always use load_file_in_new_tab which has built-in duplicate detection
                                // If the file is already open, it will switch to that tab instead
                                self.load_file_in_new_tab(&file_path)?;
                            } else {
                                // Check if current tab is empty and unused
                                let current_tab = self.current_tab;
                                let current_tab_empty = {
                                    let tab = &self.tabs[current_tab];
                                    !tab.buffer.is_modified && 
                                        (tab.buffer.lines.is_empty() || 
                                         (tab.buffer.lines.len() == 1 && tab.buffer.lines[0].is_empty())) &&
                                        (tab.buffer.file_path.is_none() || 
                                         tab.buffer.file_path.as_ref().unwrap().starts_with("untitled-"))
                                };
                                
                                // If the current tab is empty, load directly in this tab
                                if current_tab_empty {
                                    self.load_file(&file_path)?;
                                } else {
                                    // Otherwise, use load_file_in_new_tab which has built-in duplicate detection
                                    // This either switches to an existing tab with this file or loads it in a new tab
                                    self.load_file_in_new_tab(&file_path)?;
                                }
                                
                                // Note: No need to close the empty tab, as we now use it directly
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
                    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                        // Always use load_file_in_new_tab which has built-in duplicate detection
                        // If the file is already open, it will switch to that tab instead
                        self.load_file_in_new_tab(&file_path)?;
                    } else {
                        // Check if current tab is empty and unused
                        let current_tab = self.current_tab;
                        let current_tab_empty = {
                            let tab = &self.tabs[current_tab];
                            !tab.buffer.is_modified && 
                                (tab.buffer.lines.is_empty() || 
                                 (tab.buffer.lines.len() == 1 && tab.buffer.lines[0].is_empty())) &&
                                (tab.buffer.file_path.is_none() || 
                                 tab.buffer.file_path.as_ref().unwrap().starts_with("untitled-"))
                        };
                        
                        // If the current tab is empty, load directly in this tab
                        if current_tab_empty {
                            self.load_file(&file_path)?;
                        } else {
                            // Otherwise, use load_file_in_new_tab which has built-in duplicate detection
                            // This either switches to an existing tab with this file or loads it in a new tab
                            self.load_file_in_new_tab(&file_path)?;
                        }
                        
                        // Note: No need to close the empty tab, as we now use it directly
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use crate::config::Config;
    use std::fs;
    use tempfile::tempdir;
    
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
    fn test_file_finder_empty_tab_reuse() {
        use tempfile::tempdir;
        
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // The editor starts in FileFinder mode with one empty tab
        assert_eq!(editor.mode, Mode::FileFinder);
        assert_eq!(editor.tabs.len(), 1);
        
        // Verify the first tab is empty
        let tab = &editor.tabs[0];
        assert!(!tab.buffer.is_modified);
        assert_eq!(tab.buffer.lines.len(), 1);
        assert_eq!(tab.buffer.lines[0], "");
        // The default tab has an untitled-1 filename
        assert!(tab.buffer.file_path.is_some());
        assert!(tab.buffer.file_path.as_ref().unwrap().starts_with("untitled-"));
        
        // Create a temp file to load
        let tmp_dir = tempdir().expect("Failed to create temp directory");
        let file_path = tmp_dir.path().join("test_file.txt");
        std::fs::write(&file_path, "Test content").expect("Failed to write test file");
        let file_path_str = file_path.to_str().unwrap();
        
        // Simulate selecting a file in the file finder
        // We'll use the empty tab detection logic directly to simulate what happens
        let current_tab = editor.current_tab;
        let current_tab_empty = {
            let tab = &editor.tabs[current_tab];
            !tab.buffer.is_modified && 
                (tab.buffer.lines.is_empty() || 
                    (tab.buffer.lines.len() == 1 && tab.buffer.lines[0].is_empty())) &&
                (tab.buffer.file_path.is_none() || 
                 tab.buffer.file_path.as_ref().unwrap().starts_with("untitled-"))
        };
        
        // If the current tab is empty, load directly in this tab
        if current_tab_empty {
            editor.load_file(file_path_str).expect("Failed to load file");
        } else {
            editor.load_file_in_new_tab(file_path_str).expect("Failed to load file");
        }
        
        // Verify we still have only one tab
        assert_eq!(editor.tabs.len(), 1, "New tab was created instead of reusing the empty one");
        
        // Verify the file was loaded in the current tab
        assert_eq!(editor.current_tab, 0);
        assert_eq!(editor.current_tab().buffer.get_content(), "Test content");
        assert_eq!(editor.current_tab().buffer.file_path.as_ref().unwrap(), file_path_str);
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
    
    #[test]
    fn test_delete_line_with_undo_redo() {
        // Create editor with test content
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Set to Normal mode (editor starts in FileFinder mode by default)
        editor.mode = Mode::Normal;
        
        // Setup buffer with 3 lines
        editor.current_tab_mut().buffer.lines = vec![
            "First line".to_string(),
            "Second line".to_string(),
            "Third line".to_string(),
        ];
        
        // Move cursor to second line
        editor.current_tab_mut().cursor.y = 1;
        editor.current_tab_mut().cursor.x = 0;
        
        // Delete the line directly (instead of simulating key press)
        editor.current_tab_mut().buffer.delete_line(1);
        
        // Verify line was deleted
        assert_eq!(editor.current_tab().buffer.lines.len(), 2);
        assert_eq!(editor.current_tab().buffer.lines[0], "First line");
        assert_eq!(editor.current_tab().buffer.lines[1], "Third line");
        
        // Call undo on buffer
        {
            let tab = editor.current_tab_mut();
            let undo_successful = tab.buffer.undo(&mut tab.cursor);
            assert!(undo_successful);
        }
        
        // Verify the line was restored
        assert_eq!(editor.current_tab().buffer.lines.len(), 3);
        assert_eq!(editor.current_tab().buffer.lines[0], "First line");
        assert_eq!(editor.current_tab().buffer.lines[1], "Second line");
        assert_eq!(editor.current_tab().buffer.lines[2], "Third line");
        
        // Call redo on buffer
        {
            let tab = editor.current_tab_mut();
            let redo_successful = tab.buffer.redo(&mut tab.cursor);
            assert!(redo_successful);
        }
        
        // Verify the line was deleted again
        assert_eq!(editor.current_tab().buffer.lines.len(), 2);
        assert_eq!(editor.current_tab().buffer.lines[0], "First line");
        assert_eq!(editor.current_tab().buffer.lines[1], "Third line");
    }
    
    #[test]
    fn test_delete_mode_functionality() {
        // Create editor with some content
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Set to Normal mode
        editor.mode = Mode::Normal;
        
        // Setup buffer with test content
        editor.current_tab_mut().buffer.lines = vec![
            "First line of text".to_string(),
            "Second line of text".to_string(),
            "Third line with some words".to_string()
        ];
        
        // Set cursor to start of second line
        editor.current_tab_mut().cursor.y = 1;
        editor.current_tab_mut().cursor.x = 0;
        
        // Press 'd' in normal mode should enter delete mode
        let d_key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
        let _ = editor.handle_key(d_key);
        
        // Verify we entered delete mode
        assert_eq!(editor.mode, Mode::Delete);
        
        // Press 'd' again to delete the line
        let _ = editor.handle_key(d_key);
        
        // Verify the line was deleted
        assert_eq!(editor.current_tab().buffer.lines.len(), 2);
        assert_eq!(editor.current_tab().buffer.lines[0], "First line of text");
        assert_eq!(editor.current_tab().buffer.lines[1], "Third line with some words");
        
        // Verify we're back in normal mode
        assert_eq!(editor.mode, Mode::Normal);
        
        // Test word deletion (dw)
        editor.current_tab_mut().cursor.y = 0;
        editor.current_tab_mut().cursor.x = 0;
        
        // Press 'd' to enter delete mode
        let _ = editor.handle_key(d_key);
        assert_eq!(editor.mode, Mode::Delete);
        
        // Press 'w' to delete word
        let w_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::empty());
        let _ = editor.handle_key(w_key);
        
        // Verify the word was deleted (note: there may be a space before "line")
        assert!(editor.current_tab().buffer.lines[0].trim() == "line of text");
        
        // Verify we're back in normal mode
        assert_eq!(editor.mode, Mode::Normal);
    }
    
    #[test]
    fn test_mouse_scroll_handling() {
        use crossterm::event::{MouseEvent, MouseEventKind};
        
        // Create editor with some content
        let config = Config::default();
        let mut editor = Editor::new_with_config(config);
        
        // Setup buffer with multiple lines
        editor.current_tab_mut().buffer.lines = vec![
            "Line 1".to_string(),
            "Line 2".to_string(),
            "Line 3".to_string(),
            "Line 4".to_string(),
            "Line 5".to_string(),
            "Line 6".to_string(),
            "Line 7".to_string(),
            "Line 8".to_string(),
            "Line 9".to_string(),
            "Line 10".to_string(),
            "Line 11".to_string(),
            "Line 12".to_string(),
            "Line 13".to_string(),
            "Line 14".to_string(),
            "Line 15".to_string(),
        ];
        
        // Initial viewport should be at top
        assert_eq!(editor.current_tab().viewport.top_line, 0);
        
        // Create a scroll down mouse event (no column/row values needed for test)
        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        
        // Handle the mouse scroll down event
        let _ = editor.handle_mouse(scroll_down);
        
        // Verify viewport scrolled down by 3 lines
        assert_eq!(editor.current_tab().viewport.top_line, 3);
        
        // Create a scroll up mouse event
        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::empty(),
        };
        
        // Handle the mouse scroll up event
        let _ = editor.handle_mouse(scroll_up);
        
        // Verify viewport scrolled up by 3 lines
        assert_eq!(editor.current_tab().viewport.top_line, 0);
        
        // Test with empty tab list
        let mut empty_editor = Editor::new_with_config(Config::default());
        empty_editor.tabs.clear();
        
        // Should not panic with empty tab list
        let result = empty_editor.handle_mouse(scroll_down);
        assert!(result.is_ok());
    }
}