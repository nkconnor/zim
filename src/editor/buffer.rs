use anyhow::{Context, Result};
use std::fs;
use std::collections::HashSet;
use super::cursor::Cursor;
use similar::{ChangeTag, TextDiff};
use syntect::parsing::SyntaxReference;
use std::sync::Arc;
use std::cmp::{min, max};
use super::history::{History, EditorAction, ActionType};

pub struct Buffer {
    pub lines: Vec<String>,
    pub file_path: Option<String>,
    pub modified_lines: HashSet<usize>,
    pub is_modified: bool,
    pub syntax: Option<Arc<SyntaxReference>>,
    pub selection_start: Option<(usize, usize)>, // (line, column)
    pub history: History,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            file_path: None,
            modified_lines: HashSet::new(),
            is_modified: false,
            syntax: None,
            selection_start: None,
            history: History::new(),
        }
    }
    
    /// Start a selection at the current cursor position
    pub fn start_selection(&mut self, position: (usize, usize)) {
        self.selection_start = Some(position);
    }
    
    /// Undo the last action
    pub fn undo(&mut self, cursor: &mut Cursor) -> bool {
        self.history.start_undo_or_redo();
        
        let result = if let Some(action) = self.history.undo_action() {
            // Store the cursor position from before the action
            *cursor = action.cursor_before;
            
            match action.action_type {
                ActionType::InsertChar { x, y, c: _ } => {
                    // To undo an insert, we delete the character
                    if y < self.lines.len() {
                        let line = &mut self.lines[y];
                        if x < line.len() {
                            line.remove(x);
                            self.mark_line_modified(y);
                            self.is_modified = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::DeleteChar { x, y, c } => {
                    // To undo a delete, we insert the character
                    if y < self.lines.len() {
                        let line = &mut self.lines[y];
                        if x <= line.len() {
                            line.insert(x, c);
                            self.mark_line_modified(y);
                            self.is_modified = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::InsertNewline { x: _, y, remaining_text } => {
                    // To undo a newline, we join the current line with the next one
                    if y < self.lines.len() && y + 1 < self.lines.len() {
                        let current_line = &mut self.lines[y];
                        // Append the remaining text back to the line
                        *current_line = format!("{}{}", current_line, remaining_text);
                        
                        // Remove the next line
                        self.lines.remove(y + 1);
                        
                        self.mark_line_modified(y);
                        self.is_modified = true;
                        true
                    } else {
                        false
                    }
                },
                ActionType::DeleteLine { y, content } => {
                    // To undo a line deletion, we insert the line back
                    if y <= self.lines.len() {
                        self.lines.insert(y, content);
                        self.mark_line_modified(y);
                        self.is_modified = true;
                        true
                    } else {
                        false
                    }
                },
                ActionType::JoinLines { y, column_pos } => {
                    // To undo joining lines, we split the line again
                    if y < self.lines.len() {
                        // Make a copy of the line to avoid borrowing issues
                        let line = self.lines[y].clone();
                        if column_pos <= line.len() {
                            let (before, after) = line.split_at(column_pos);
                            self.lines[y] = before.to_string();
                            self.lines.insert(y + 1, after.to_string());
                            self.mark_line_modified(y);
                            self.mark_line_modified(y + 1);
                            self.is_modified = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::ReplaceSelection { old_text, selection_start, selection_end, .. } => {
                    // To undo a selection replacement, we need to:
                    // 1. Delete the current text in the selection area
                    // 2. Insert the original text
                    
                    let (start_line, start_col) = selection_start;
                    let (end_line, end_col) = selection_end;
                    
                    // Set the cursor to the beginning of the selection
                    cursor.y = start_line;
                    cursor.x = start_col;
                    
                    if self.delete_between(start_line, start_col, end_line, end_col) {
                        // Now insert the original text
                        self.insert_text_at_cursor(old_text, cursor);
                        self.is_modified = true;
                        true
                    } else {
                        false
                    }
                },
                ActionType::SetContent { old_lines, .. } => {
                    // To undo setting content, we restore the old lines
                    self.lines = old_lines;
                    
                    // Mark all lines as modified
                    self.modified_lines.clear();
                    for i in 0..self.lines.len() {
                        self.modified_lines.insert(i);
                    }
                    
                    self.is_modified = true;
                    true
                },
                ActionType::OpenLineBelow { y } => {
                    // To undo opening a line below, remove the line
                    if y + 1 < self.lines.len() {
                        self.lines.remove(y + 1);
                        self.mark_line_modified(y);
                        self.is_modified = true;
                        true
                    } else {
                        false
                    }
                },
                ActionType::OpenLineAbove { y } => {
                    // To undo opening a line above, remove the line
                    if y < self.lines.len() {
                        self.lines.remove(y);
                        if y < self.lines.len() {
                            self.mark_line_modified(y);
                        }
                        self.is_modified = true;
                        true
                    } else {
                        false
                    }
                },
                ActionType::DeleteWord { position, deleted_text } => {
                    // To undo word deletion, we insert the word back
                    let (y, x) = position;
                    if y < self.lines.len() {
                        let line = &mut self.lines[y];
                        if x <= line.len() {
                            // Insert the deleted text back at the position
                            let before = &line[..x];
                            let after = &line[x..];
                            *line = format!("{}{}{}", before, deleted_text, after);
                            self.mark_line_modified(y);
                            self.is_modified = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::DeleteToEndOfLine { position, deleted_text } => {
                    // To undo end-of-line deletion, we append the deleted text back
                    let (y, x) = position;
                    if y < self.lines.len() {
                        let line = &mut self.lines[y];
                        if x <= line.len() {
                            line.push_str(&deleted_text);
                            self.mark_line_modified(y);
                            self.is_modified = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::DeleteToStartOfLine { position, deleted_text } => {
                    // To undo start-of-line deletion, we prepend the deleted text back
                    let (y, _) = position;
                    if y < self.lines.len() {
                        let line = &mut self.lines[y];
                        // Create a new line with the prepended text
                        *line = format!("{}{}", deleted_text, line);
                        self.mark_line_modified(y);
                        self.is_modified = true;
                        true
                    } else {
                        false
                    }
                },
            }
        } else {
            false
        };
        
        self.history.end_undo_or_redo();
        result
    }
    
    /// Redo the previously undone action
    pub fn redo(&mut self, cursor: &mut Cursor) -> bool {
        self.history.start_undo_or_redo();
        
        let result = if let Some(action) = self.history.redo_action() {
            // Set the cursor to the position from before the action
            *cursor = action.cursor_before;
            
            match action.action_type {
                ActionType::InsertChar { x, y, c } => {
                    // To redo an insert, we insert the character again
                    if y < self.lines.len() {
                        let line = &mut self.lines[y];
                        if x <= line.len() {
                            line.insert(x, c);
                            self.mark_line_modified(y);
                            self.is_modified = true;
                            
                            // Update cursor position
                            cursor.x = x + 1;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::DeleteChar { x, y, .. } => {
                    // To redo a delete, we delete the character again
                    if y < self.lines.len() {
                        let line = &mut self.lines[y];
                        if x < line.len() {
                            line.remove(x);
                            self.mark_line_modified(y);
                            self.is_modified = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::InsertNewline { x, y, .. } => {
                    // To redo a newline, we split the line again
                    if y < self.lines.len() {
                        let line = &self.lines[y].clone();
                        if x <= line.len() {
                            let (before, after) = line.split_at(x);
                            self.lines[y] = before.to_string();
                            self.lines.insert(y + 1, after.to_string());
                            self.mark_line_modified(y);
                            self.mark_line_modified(y + 1);
                            self.is_modified = true;
                            
                            // Update cursor position
                            cursor.y = y + 1;
                            cursor.x = 0;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::DeleteLine { y, .. } => {
                    // To redo a line deletion, we delete the line again
                    if y < self.lines.len() {
                        self.lines.remove(y);
                        if y < self.lines.len() {
                            self.mark_line_modified(y);
                        }
                        self.is_modified = true;
                        
                        // Adjust cursor if needed
                        if cursor.y >= self.lines.len() {
                            cursor.y = self.lines.len().saturating_sub(1);
                            cursor.x = self.line_length(cursor.y);
                        }
                        
                        true
                    } else {
                        false
                    }
                },
                ActionType::JoinLines { y, column_pos } => {
                    // To redo joining lines, we join them again
                    if y < self.lines.len() && y + 1 < self.lines.len() {
                        let next_line = self.lines.remove(y + 1);
                        self.lines[y].push_str(&next_line);
                        self.mark_line_modified(y);
                        self.is_modified = true;
                        
                        // Update cursor position
                        cursor.x = column_pos;
                        true
                    } else {
                        false
                    }
                },
                ActionType::ReplaceSelection { new_text, selection_start, selection_end, .. } => {
                    // To redo a selection replacement, we:
                    // 1. Delete the text in the selection area again
                    // 2. Insert the new text
                    
                    let (start_line, start_col) = selection_start;
                    let (end_line, end_col) = selection_end;
                    
                    // Set cursor to the beginning of the selection
                    cursor.y = start_line;
                    cursor.x = start_col;
                    
                    if self.delete_between(start_line, start_col, end_line, end_col) {
                        // Now insert the new text
                        self.insert_text_at_cursor(new_text, cursor);
                        self.is_modified = true;
                        true
                    } else {
                        false
                    }
                },
                ActionType::SetContent { new_lines, .. } => {
                    // To redo setting content, we restore the new lines
                    self.lines = new_lines;
                    
                    // Mark all lines as modified
                    self.modified_lines.clear();
                    for i in 0..self.lines.len() {
                        self.modified_lines.insert(i);
                    }
                    
                    self.is_modified = true;
                    true
                },
                ActionType::OpenLineBelow { y } => {
                    // To redo opening a line below, insert an empty line
                    if y < self.lines.len() {
                        self.lines.insert(y + 1, String::new());
                        self.mark_line_modified(y + 1);
                        self.is_modified = true;
                        
                        // Update cursor position
                        cursor.y = y + 1;
                        cursor.x = 0;
                        true
                    } else {
                        false
                    }
                },
                ActionType::OpenLineAbove { y } => {
                    // To redo opening a line above, insert an empty line
                    if y <= self.lines.len() {
                        self.lines.insert(y, String::new());
                        self.mark_line_modified(y);
                        self.is_modified = true;
                        
                        // Update cursor position
                        cursor.y = y;
                        cursor.x = 0;
                        true
                    } else {
                        false
                    }
                },
                ActionType::DeleteWord { position, deleted_text } => {
                    // To redo word deletion, remove the text again
                    let (line_idx, col_idx) = position;
                    if line_idx < self.lines.len() {
                        let line = &mut self.lines[line_idx];
                        if col_idx + deleted_text.len() <= line.len() {
                            // Check if the word is still there
                            let word_to_delete = &line[col_idx..col_idx + deleted_text.len()];
                            if word_to_delete == deleted_text {
                                // Remove it
                                let new_line = format!("{}{}", &line[..col_idx], &line[col_idx + deleted_text.len()..]);
                                *line = new_line;
                                self.mark_line_modified(line_idx);
                                self.is_modified = true;
                                
                                // Update cursor position
                                cursor.y = line_idx;
                                cursor.x = col_idx;
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::DeleteToEndOfLine { position, deleted_text: _ } => {
                    // To redo end-of-line deletion, truncate the line again
                    let (line_idx, col_idx) = position;
                    if line_idx < self.lines.len() {
                        let line = &mut self.lines[line_idx];
                        if col_idx <= line.len() {
                            // Truncate the line at column position
                            line.truncate(col_idx);
                            self.mark_line_modified(line_idx);
                            self.is_modified = true;
                            
                            // Update cursor position
                            cursor.y = line_idx;
                            cursor.x = col_idx;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
                ActionType::DeleteToStartOfLine { position, deleted_text: _ } => {
                    // To redo start-of-line deletion, remove from start of line again
                    let (line_idx, col_idx) = position;
                    if line_idx < self.lines.len() {
                        let line = &mut self.lines[line_idx];
                        if col_idx <= line.len() {
                            // Remove characters from start of line
                            *line = line[col_idx.min(line.len())..].to_string();
                            self.mark_line_modified(line_idx);
                            self.is_modified = true;
                            
                            // Update cursor position
                            cursor.y = line_idx;
                            cursor.x = 0;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                },
            }
        } else {
            false
        };
        
        // Update the cursor to the after-position
        if result {
            cursor.x = min(cursor.x, self.line_length(cursor.y));
        }
        
        self.history.end_undo_or_redo();
        result
    }
    
    /// Helper function to delete text between two positions
    fn delete_between(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> bool {
        if start_line >= self.lines.len() || end_line >= self.lines.len() {
            return false;
        }
        
        if start_line == end_line {
            // Delete within a single line
            let line = &mut self.lines[start_line];
            if start_col <= line.len() && end_col <= line.len() && start_col <= end_col {
                let before = &line[..start_col];
                let after = &line[end_col..];
                *line = format!("{}{}", before, after);
                self.mark_line_modified(start_line);
                return true;
            }
        } else if start_line < end_line {
            // Delete across multiple lines
            
            // Get the beginning of the first line and the end of the last line
            let first_line = &self.lines[start_line];
            let last_line = &self.lines[end_line];
            
            if start_col <= first_line.len() && end_col <= last_line.len() {
                let first_part = &first_line[..start_col];
                let last_part = &last_line[end_col..];
                
                // Create the new merged line
                let new_line = format!("{}{}", first_part, last_part);
                
                // Replace the first line with the merged content
                self.lines[start_line] = new_line;
                
                // Remove all lines in between
                self.lines.drain(start_line + 1..=end_line);
                
                // Mark the modified line
                self.mark_line_modified(start_line);
                
                return true;
            }
        }
        
        false
    }
    
    /// Helper function to insert text at the cursor position
    fn insert_text_at_cursor(&mut self, text: String, cursor: &mut Cursor) {
        if text.is_empty() {
            return;
        }
        
        // Split the text into lines
        let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        
        // If there are no newlines, just insert the text at the cursor
        if lines.is_empty() {
            lines.push(text);
        }
        
        if lines.len() == 1 {
            // Simple case: insert a single line
            if cursor.y < self.lines.len() {
                let line = &mut self.lines[cursor.y];
                if cursor.x <= line.len() {
                    let before = line[..cursor.x].to_string();
                    let after = line[cursor.x..].to_string();
                    *line = format!("{}{}{}", before, lines[0], after);
                    
                    // Update cursor position
                    cursor.x += lines[0].len();
                    
                    self.mark_line_modified(cursor.y);
                }
            }
        } else {
            // Multi-line insertion
            if cursor.y < self.lines.len() {
                // Make copy of current line to avoid borrow issues
                let current_line = self.lines[cursor.y].clone();
                
                if cursor.x <= current_line.len() {
                    // Split the current line
                    let before = &current_line[..cursor.x];
                    let after = &current_line[cursor.x..];
                    
                    // Update the first line with the beginning + first inserted line
                    self.lines[cursor.y] = format!("{}{}", before, lines[0]);
                    self.mark_line_modified(cursor.y);
                    
                    // Insert middle lines
                    for (i, middle_line) in lines.iter().skip(1).take(lines.len() - 2).enumerate() {
                        self.lines.insert(cursor.y + 1 + i, middle_line.clone());
                        self.mark_line_modified(cursor.y + 1 + i);
                    }
                    
                    // Insert the last line + end of original line
                    if lines.len() >= 2 {
                        let last_inserted = &lines[lines.len() - 1];
                        self.lines.insert(cursor.y + lines.len() - 1, format!("{}{}", last_inserted, after));
                        self.mark_line_modified(cursor.y + lines.len() - 1);
                    }
                    
                    // Update cursor position
                    cursor.y += lines.len() - 1;
                    cursor.x = lines.last().unwrap().len();
                }
            }
        }
    }
    
    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
    }
    
    /// Check if a position is within the current selection
    /// 
    /// For Visual mode, the selection extends from the start position to the cursor
    /// For VisualLine mode, the selection extends to include entire lines from start to end
    pub fn is_position_selected(&self, line: usize, column: usize, cursor: &Cursor, is_visual_line: bool) -> bool {
        if let Some(start) = self.selection_start {
            let (start_line, start_col) = start;
            let (end_line, end_col) = (cursor.y, cursor.x);
            
            // Ensure start is before end
            let (first_line, first_col, last_line, last_col) = if start_line < end_line || 
                (start_line == end_line && start_col <= end_col) {
                (start_line, start_col, end_line, end_col)
            } else {
                (end_line, end_col, start_line, start_col)
            };
            
            // Check if the position is within the selection range
            if line < first_line || line > last_line {
                return false;
            }
            
            // For VisualLine mode, select entire lines regardless of column
            if is_visual_line {
                return true;
            }
            
            // For Visual mode, handle column ranges
            if line == first_line && line == last_line {
                return column >= first_col && column < last_col;
            } else if line == first_line {
                return column >= first_col;
            } else if line == last_line {
                return column < last_col;
            }
            
            return true;
        }
        
        false
    }
    
    /// Get the selected text
    /// 
    /// If is_visual_line is true, entire lines will be selected regardless of cursor column
    pub fn get_selected_text(&self, cursor: &Cursor, is_visual_line: bool) -> String {
        if let Some(start) = self.selection_start {
            let (start_line, start_col) = start;
            let (end_line, end_col) = (cursor.y, cursor.x);
            
            // Ensure start is before end
            let (first_line, first_col, last_line, last_col) = if start_line < end_line || 
                (start_line == end_line && start_col <= end_col) {
                (start_line, start_col, end_line, end_col)
            } else {
                (end_line, end_col, start_line, start_col)
            };
            
            // For visual line mode, select entire lines
            if is_visual_line {
                let mut result = String::new();
                
                // Add all selected lines in their entirety
                for line_idx in first_line..=last_line {
                    if line_idx < self.lines.len() {
                        result.push_str(&self.lines[line_idx]);
                        // Always add a newline after each line in VisualLine mode
                        result.push('\n');
                    }
                }
                
                // Ensure the result always ends with a newline in VisualLine mode
                // (This guarantees proper line-based pasting behavior)
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                
                return result;
            }
            
            // Handle character-based selection on a single line
            if first_line == last_line {
                if first_line < self.lines.len() {
                    let line = &self.lines[first_line];
                    let end_col = min(last_col, line.len());
                    let start_col = min(first_col, line.len());
                    if start_col <= end_col && start_col < line.len() {
                        // If the selection extends to the end of the line and it's not the last line,
                        // include the newline character to maintain proper line breaks when pasting
                        let content = line[start_col..end_col].to_string();
                        if end_col >= line.len() && first_line < self.lines.len() - 1 {
                            return format!("{}\n", content);
                        }
                        return content;
                    }
                }
                return String::new();
            }
            
            // Handle multi-line character-based selection
            let mut result = String::new();
            
            // First line from start_col to end
            if first_line < self.lines.len() {
                let line = &self.lines[first_line];
                let start_col = min(first_col, line.len());
                if start_col < line.len() {
                    result.push_str(&line[start_col..]);
                }
                result.push('\n');
            }
            
            // Middle lines in their entirety
            for line_idx in first_line+1..last_line {
                if line_idx < self.lines.len() {
                    result.push_str(&self.lines[line_idx]);
                    result.push('\n');
                }
            }
            
            // Last line from start to end_col
            if last_line < self.lines.len() {
                let line = &self.lines[last_line];
                let end_col = min(last_col, line.len());
                if end_col > 0 {
                    result.push_str(&line[..end_col]);
                }
                
                // Include a final newline if the selection extends to the end of line
                // and it's not the very last line of the buffer
                if end_col >= line.len() && last_line < self.lines.len() - 1 {
                    result.push('\n');
                }
            }
            
            return result;
        }
        
        String::new()
    }
    
    /// Delete the selected text and return true if deletion was performed
    /// 
    /// If is_visual_line is true, entire lines will be deleted regardless of cursor column
    pub fn delete_selection(&mut self, cursor: &mut Cursor, is_visual_line: bool) -> bool {
        if let Some(start) = self.selection_start {
            let (start_line, start_col) = start;
            let (end_line, end_col) = (cursor.y, cursor.x);
            
            // Create the action before modifying the buffer
            let cursor_before = *cursor;
            
            // Ensure start is before end
            let (first_line, first_col, last_line, last_col) = if start_line < end_line || 
                (start_line == end_line && start_col <= end_col) {
                (start_line, start_col, end_line, end_col)
            } else {
                (end_line, end_col, start_line, start_col)
            };
            
            // Get the selected text before modifying the buffer
            let selected_text = self.get_selected_text(cursor, is_visual_line);
            
            // Handle visual line mode (delete entire lines)
            if is_visual_line {
                if first_line < self.lines.len() {
                    // Delete the selected lines
                    if first_line <= last_line && last_line < self.lines.len() {
                        // Remove lines from first to last inclusive
                        let lines_to_delete = last_line - first_line + 1;
                        if lines_to_delete < self.lines.len() {
                            // Don't remove all lines, leave at least one
                            self.lines.drain(first_line..=last_line);
                            
                            // If we removed all lines, add an empty line
                            if self.lines.is_empty() {
                                self.lines.push(String::new());
                            }
                        } else {
                            // Delete all but one line, make it empty
                            self.lines = vec![String::new()];
                        }
                        
                        // Mark as modified
                        self.is_modified = true;
                        
                        // Update modified line indices
                        let removed_line_count = last_line - first_line + 1;
                        let mut new_modified_lines = HashSet::new();
                        for &line_idx in &self.modified_lines {
                            if line_idx < first_line {
                                new_modified_lines.insert(line_idx);
                            } else if line_idx > last_line {
                                new_modified_lines.insert(line_idx - removed_line_count);
                            }
                        }
                        self.modified_lines = new_modified_lines;
                        
                        // Move cursor to beginning of first line
                        cursor.y = min(first_line, self.lines.len().saturating_sub(1));
                        cursor.x = 0;
                        
                        // Record the action in history
                        let cursor_after = *cursor;
                        self.history.push(EditorAction {
                            action_type: ActionType::ReplaceSelection { 
                                old_text: selected_text, 
                                new_text: String::new(),
                                selection_start: (first_line, first_col),
                                selection_end: (last_line, last_col),
                            },
                            cursor_before,
                            cursor_after,
                        });
                        
                        // Clear selection
                        self.selection_start = None;
                        return true;
                    }
                }
            }
            // Handle character-based selection on a single line
            else if first_line == last_line && first_line < self.lines.len() {
                let line = &mut self.lines[first_line];
                let end_col = min(last_col, line.len());
                let start_col = min(first_col, line.len());
                
                if start_col < end_col && start_col < line.len() {
                    // Remove the selected portion of the line
                    let suffix = if end_col < line.len() {
                        line[end_col..].to_string()
                    } else {
                        String::new()
                    };
                    
                    line.truncate(start_col);
                    line.push_str(&suffix);
                    
                    // Mark line as modified
                    self.modified_lines.insert(first_line);
                    self.is_modified = true;
                    
                    // Move cursor to selection start
                    cursor.y = first_line;
                    cursor.x = start_col;
                    
                    // Record the action in history
                    let cursor_after = *cursor;
                    self.history.push(EditorAction {
                        action_type: ActionType::ReplaceSelection { 
                            old_text: selected_text, 
                            new_text: String::new(),
                            selection_start: (first_line, first_col),
                            selection_end: (last_line, last_col),
                        },
                        cursor_before,
                        cursor_after,
                    });
                    
                    // Clear selection
                    self.selection_start = None;
                    return true;
                }
            } else if first_line < self.lines.len() && last_line < self.lines.len() {
                // First line: keep from start to first_col
                let first_line_prefix = if first_col <= self.lines[first_line].len() {
                    self.lines[first_line][..first_col].to_string()
                } else {
                    self.lines[first_line].clone()
                };
                
                // Last line: keep from last_col to end
                let last_line_suffix = if last_col < self.lines[last_line].len() {
                    self.lines[last_line][last_col..].to_string()
                } else {
                    String::new()
                };
                
                // Join the first line prefix with the last line suffix
                self.lines[first_line] = first_line_prefix + &last_line_suffix;
                
                // Remove all lines between first_line+1 and last_line inclusive
                if first_line + 1 <= last_line && last_line < self.lines.len() {
                    self.lines.drain(first_line+1..=last_line);
                }
                
                // Mark lines as modified
                self.modified_lines.insert(first_line);
                self.is_modified = true;
                
                // Update modified line indices
                let removed_line_count = last_line - first_line;
                let mut new_modified_lines = HashSet::new();
                for &line_idx in &self.modified_lines {
                    if line_idx <= first_line {
                        new_modified_lines.insert(line_idx);
                    } else if line_idx > last_line {
                        new_modified_lines.insert(line_idx - removed_line_count);
                    }
                }
                self.modified_lines = new_modified_lines;
                
                // Move cursor to selection start
                cursor.y = first_line;
                cursor.x = first_col;
                
                // Record the action in history
                let cursor_after = *cursor;
                self.history.push(EditorAction {
                    action_type: ActionType::ReplaceSelection { 
                        old_text: selected_text, 
                        new_text: String::new(),
                        selection_start: (first_line, first_col),
                        selection_end: (last_line, last_col),
                    },
                    cursor_before,
                    cursor_after,
                });
                
                // Clear selection
                self.selection_start = None;
                return true;
            }
            
            // Clear selection even if we didn't delete anything
            self.selection_start = None;
        }
        
        false
    }
    
    /// Set the syntax for this buffer
    pub fn set_syntax(&mut self, syntax: Option<Arc<SyntaxReference>>) {
        self.syntax = syntax;
    }

    /// Load a file into the buffer with optimized line handling
    /// 
    /// This implementation uses Rust's built-in line iterator for more readable
    /// and potentially more efficient line handling.
    pub fn load_file(&mut self, path: &str) -> Result<()> {
        // Read the file content
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        
        // Calculate an approximate capacity to reduce reallocations
        // Assume average of 40 chars per line as a heuristic
        let estimated_line_count = content.len() / 40 + 1;
        
        // Start fresh with pre-allocated lines vector
        self.lines = Vec::with_capacity(estimated_line_count);
        
        // Handle empty file case
        if content.is_empty() {
            self.lines.push(String::new());
        } else {
            // Use lines() iterator which properly handles different line endings
            // and is more readable than manual character iteration
            for line in content.lines() {
                self.lines.push(line.to_string());
            }
            
            // If the file ends with a newline, add an empty line at the end
            if content.ends_with('\n') {
                self.lines.push(String::new());
            }
        }
        
        // Store the file path
        self.file_path = Some(path.to_string());
        
        // Clear modification state
        self.modified_lines.clear();
        self.is_modified = false;
        
        Ok(())
    }

    pub fn insert_char_at_cursor(&mut self, c: char, cursor: &Cursor) {
        // Create the action before modifying the buffer
        let cursor_before = *cursor;
        let cursor_after = Cursor { x: cursor.x + 1, y: cursor.y };
        
        if cursor.y >= self.lines.len() {
            // Add empty lines if cursor is beyond current lines
            while cursor.y >= self.lines.len() {
                self.lines.push(String::new());
                self.modified_lines.insert(self.lines.len() - 1);
            }
        }

        let line = &mut self.lines[cursor.y];
        
        // Reserve space if we're going to grow the line
        if cursor.x > line.len() {
            // Pre-allocate line capacity for better performance
            let needed_capacity = cursor.x + 1;
            if line.capacity() < needed_capacity {
                line.reserve(needed_capacity - line.len());
            }
            
            // Pad with spaces if cursor is beyond the end of the line
            while cursor.x > line.len() {
                line.push(' ');
            }
            line.push(c);
        } else {
            // Insert character at cursor position
            line.insert(cursor.x, c);
        }
        
        // Mark line as modified
        self.modified_lines.insert(cursor.y);
        self.is_modified = true;
        
        // Record the action in history
        self.history.push(EditorAction {
            action_type: ActionType::InsertChar { x: cursor.x, y: cursor.y, c },
            cursor_before,
            cursor_after,
        });
    }

    pub fn delete_char_at_cursor(&mut self, cursor: &Cursor) {
        // Create the action before modifying the buffer
        let cursor_before = *cursor;
        let cursor_after = *cursor; // Cursor doesn't move after delete
        
        if cursor.y < self.lines.len() {
            let line = &mut self.lines[cursor.y];
            if cursor.x < line.len() {
                // Store the character being deleted
                let c = line.chars().nth(cursor.x).unwrap();
                
                line.remove(cursor.x);
                // Mark line as modified
                self.modified_lines.insert(cursor.y);
                self.is_modified = true;
                
                // Record the action in history
                self.history.push(EditorAction {
                    action_type: ActionType::DeleteChar { x: cursor.x, y: cursor.y, c },
                    cursor_before,
                    cursor_after,
                });
            }
        }
    }
    
    pub fn delete_line(&mut self, line_idx: usize) {
        if line_idx < self.lines.len() {
            // Save line content before deleting
            let content = self.lines[line_idx].clone();
            
            // Create the action before modifying the buffer
            let cursor_before = Cursor { x: 0, y: line_idx };
            let cursor_after = if line_idx + 1 < self.lines.len() {
                Cursor { x: 0, y: line_idx }
            } else if line_idx > 0 {
                Cursor { x: self.lines[line_idx - 1].len(), y: line_idx - 1 }
            } else {
                Cursor { x: 0, y: 0 }
            };
            
            // Remove the line
            self.lines.remove(line_idx);
            
            // If this was the last line, add an empty line to ensure buffer always has at least one line
            if self.lines.is_empty() {
                self.lines.push(String::new());
            }
            
            // Mark buffer as modified
            self.is_modified = true;
            
            // Update modified lines tracking - shift all line numbers above the deleted line
            let mut new_modified_lines = HashSet::new();
            for &modified_line in &self.modified_lines {
                if modified_line < line_idx {
                    // Lines before the deleted line keep their indices
                    new_modified_lines.insert(modified_line);
                } else if modified_line > line_idx {
                    // Lines after the deleted line have indices shifted down by 1
                    new_modified_lines.insert(modified_line - 1);
                }
                // The deleted line itself is removed from tracking
            }
            self.modified_lines = new_modified_lines;
            
            // Record the action in history
            self.history.push(EditorAction {
                action_type: ActionType::DeleteLine { y: line_idx, content },
                cursor_before,
                cursor_after,
            });
        }
    }
    
    pub fn join_line(&mut self, line_idx: usize) {
        // Join current line with the next line
        if line_idx < self.lines.len() - 1 {
            // Save info for the history
            let column_pos = self.lines[line_idx].len();
            
            // Create the action before modifying the buffer
            let cursor_before = Cursor { x: column_pos, y: line_idx };
            let cursor_after = Cursor { x: column_pos, y: line_idx };
            
            // Get the content of the next line
            let next_line_content = self.lines[line_idx + 1].clone();
            
            // Append it to the current line
            self.lines[line_idx].push_str(&next_line_content);
            
            // Remove the next line
            self.lines.remove(line_idx + 1);
            
            // Mark lines as modified
            self.modified_lines.insert(line_idx);
            self.is_modified = true;
            
            // Update modified lines tracking - shift indices above the removed line
            let mut new_modified_lines = HashSet::new();
            for &modified_line in &self.modified_lines {
                if modified_line <= line_idx {
                    // Lines up to the joined line keep their indices
                    new_modified_lines.insert(modified_line);
                } else if modified_line > line_idx + 1 {
                    // Lines after the deleted line have indices shifted down by 1
                    new_modified_lines.insert(modified_line - 1);
                }
                // The deleted line itself is removed from tracking
            }
            self.modified_lines = new_modified_lines;
            
            // Record the action in history
            self.history.push(EditorAction {
                action_type: ActionType::JoinLines { y: line_idx, column_pos },
                cursor_before,
                cursor_after,
            });
        }
    }

    pub fn insert_newline_at_cursor(&mut self, cursor: &Cursor) {
        // Create the action before modifying the buffer
        let cursor_before = *cursor;
        let cursor_after = Cursor { x: 0, y: cursor.y + 1 };
        
        if cursor.y >= self.lines.len() {
            // Add empty lines if cursor is beyond current lines
            while cursor.y >= self.lines.len() {
                self.lines.push(String::new());
                self.modified_lines.insert(self.lines.len() - 1);
            }
        }

        let line = &mut self.lines[cursor.y];
        let new_line = if cursor.x >= line.len() {
            String::new()
        } else {
            line[cursor.x..].to_string()
        };

        // Save the remaining text for history
        let remaining_text = new_line.clone();
        
        if cursor.x < line.len() {
            line.truncate(cursor.x);
        }

        self.lines.insert(cursor.y + 1, new_line);
        
        // Mark both lines as modified
        self.modified_lines.insert(cursor.y);
        self.modified_lines.insert(cursor.y + 1);
        self.is_modified = true;
        
        // Record the action in history
        self.history.push(EditorAction {
            action_type: ActionType::InsertNewline { x: cursor.x, y: cursor.y, remaining_text },
            cursor_before,
            cursor_after,
        });
    }

    /// Open a new line below the current line and return the line number
    pub fn open_line_below(&mut self, line_idx: usize) -> usize {
        // Create the action before modifying the buffer
        let cursor_before = Cursor { x: 0, y: line_idx };
        let cursor_after = Cursor { x: 0, y: line_idx + 1 };
        
        if line_idx >= self.lines.len() {
            // Add a new line at the end
            self.lines.push(String::new());
        } else {
            // Insert a new line after the current line
            self.lines.insert(line_idx + 1, String::new());
        }
        
        // Mark the new line as modified
        self.modified_lines.insert(line_idx + 1);
        self.is_modified = true;
        
        // Record the action in history
        self.history.push(EditorAction {
            action_type: ActionType::OpenLineBelow { y: line_idx },
            cursor_before,
            cursor_after,
        });
        
        // Return the index of the new line
        line_idx + 1
    }
    
    /// Open a new line above the current line and return the line number
    pub fn open_line_above(&mut self, line_idx: usize) -> usize {
        // Create the action before modifying the buffer
        let cursor_before = Cursor { x: 0, y: line_idx };
        let cursor_after = Cursor { x: 0, y: line_idx };
        
        // Insert a new line before the current line
        self.lines.insert(line_idx, String::new());
        
        // Mark the new line as modified
        self.modified_lines.insert(line_idx);
        self.is_modified = true;
        
        // Record the action in history
        self.history.push(EditorAction {
            action_type: ActionType::OpenLineAbove { y: line_idx },
            cursor_before,
            cursor_after,
        });
        
        // Return the index of the new line
        line_idx
    }

    pub fn get_line(&self, y: usize) -> &str {
        if y < self.lines.len() {
            &self.lines[y]
        } else {
            ""
        }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line_length(&self, y: usize) -> usize {
        if y < self.lines.len() {
            self.lines[y].len()
        } else {
            0
        }
    }
    
    /// Get the entire content of the buffer as a single string
    /// 
    /// This implementation is optimized to minimize allocations by pre-calculating
    /// the exact string capacity needed.
    pub fn get_content(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        
        // Calculate required capacity to avoid reallocations
        // (sum of all line lengths + newlines for each line except the last)
        let capacity = self.lines.iter().map(|line| line.len()).sum::<usize>() + self.lines.len() - 1;
        
        // Initialize with exact needed capacity
        let mut content = String::with_capacity(capacity);
        
        // Add all lines except the last one with newlines
        for i in 0..self.lines.len()-1 {
            content.push_str(&self.lines[i]);
            content.push('\n');
        }
        
        // Add the last line without a newline
        content.push_str(&self.lines[self.lines.len()-1]);
        
        content
    }
    
    /// Set the entire content of the buffer from a string
    pub fn set_content(&mut self, content: &str) -> Result<()> {
        // Save original content for history
        let old_lines = self.lines.clone();
        let cursor_before = Cursor { x: 0, y: 0 };
        
        // Start fresh with no lines
        self.lines = Vec::new();
        
        // Use the same line parsing logic as load_file for consistency
        if content.is_empty() {
            // Empty content - add a single empty line
            self.lines.push(String::new());
        } else {
            // Split at each newline, keeping track of positions
            let mut start = 0;
            
            // Process each line including the last one
            for (i, c) in content.char_indices() {
                if c == '\n' {
                    // Add the line up to the newline (but not including it)
                    self.lines.push(content[start..i].to_string());
                    start = i + 1; // Start the next line after the newline
                }
            }
            
            // Add the last line (after the last newline, or the only line if no newlines)
            if start <= content.len() {
                self.lines.push(content[start..].to_string());
            }
        }
        
        // Mark all lines as modified
        self.modified_lines.clear();
        for i in 0..self.lines.len() {
            self.modified_lines.insert(i);
        }
        self.is_modified = true;
        
        // Record the action in history
        let cursor_after = Cursor { x: 0, y: 0 };
        self.history.push(EditorAction {
            action_type: ActionType::SetContent { 
                old_lines,
                new_lines: self.lines.clone(),
            },
            cursor_before,
            cursor_after,
        });
        
        Ok(())
    }
    
    /// Save the buffer content to a file
    pub fn save(&mut self, path: Option<&str>) -> Result<String> {
        use std::fs;
        
        let file_path = match path {
            // Use provided path if given
            Some(p) => p.to_string(),
            // Otherwise use existing path or error
            None => match &self.file_path {
                Some(p) => p.clone(),
                None => return Err(anyhow::anyhow!("No file path specified")),
            },
        };
        
        // Get content and write to file
        let content = self.get_content();
        fs::write(&file_path, content)?;
        
        // Update file path if it was newly set
        if path.is_some() {
            self.file_path = Some(file_path.clone());
        }
        
        // Clear modification state after successful save
        self.modified_lines.clear();
        self.is_modified = false;
        
        // Return the path that was saved to
        Ok(file_path)
    }
    
    /// Check if line is modified
    pub fn is_line_modified(&self, line_idx: usize) -> bool {
        self.modified_lines.contains(&line_idx)
    }
    
    /// Mark a line as modified
    pub fn mark_line_modified(&mut self, line_idx: usize) {
        self.modified_lines.insert(line_idx);
        self.is_modified = true;
    }
    
    /// Get all modified line indices
    pub fn get_modified_lines(&self) -> &HashSet<usize> {
        &self.modified_lines
    }
    
    /// Delete word at cursor position
    /// Returns true if deletion was successful
    pub fn delete_word_at_cursor(&mut self, cursor: &mut Cursor) -> bool {
        // Create the action before modifying the buffer
        let cursor_before = *cursor;
        let cursor_after = *cursor; // Cursor doesn't move after delete word

        if cursor.y >= self.lines.len() {
            return false;
        }
        
        let line = &self.lines[cursor.y];
        if cursor.x >= line.len() {
            return false;
        }
        
        // Find the end of the current word
        let mut end_idx = cursor.x;
        let chars: Vec<char> = line.chars().collect();
        
        // If we're at a whitespace, delete all contiguous whitespace
        if chars[end_idx].is_whitespace() {
            while end_idx < chars.len() && chars[end_idx].is_whitespace() {
                end_idx += 1;
            }
        } 
        // If we're at a word character, delete until the next non-word char
        else if chars[end_idx].is_alphanumeric() || chars[end_idx] == '_' {
            while end_idx < chars.len() && (chars[end_idx].is_alphanumeric() || chars[end_idx] == '_') {
                end_idx += 1;
            }
        } 
        // If we're at a symbol, delete all contiguous symbols
        else {
            while end_idx < chars.len() && !(chars[end_idx].is_alphanumeric() || chars[end_idx] == '_' || chars[end_idx].is_whitespace()) {
                end_idx += 1;
            }
        }
        
        // Nothing to delete
        if end_idx == cursor.x {
            return false;
        }
        
        // Extract the text to be deleted
        let deleted_text = line[cursor.x..end_idx].to_string();
        
        // Delete the word
        let new_line = format!("{}{}", &line[0..cursor.x], &line[end_idx..]);
        self.lines[cursor.y] = new_line;
        
        // Mark line as modified
        self.mark_line_modified(cursor.y);
        self.is_modified = true;
        
        // Record the action in history
        self.history.push(EditorAction {
            action_type: ActionType::DeleteWord { 
                position: (cursor.y, cursor.x),
                deleted_text,
            },
            cursor_before,
            cursor_after,
        });
        
        true
    }
    
    /// Delete from cursor to end of line
    /// Returns true if deletion was successful
    pub fn delete_to_end_of_line(&mut self, cursor: &Cursor) -> bool {
        // Create the action before modifying the buffer
        let cursor_before = *cursor;
        let cursor_after = *cursor; // Cursor doesn't move 
        
        if cursor.y >= self.lines.len() {
            return false;
        }
        
        let line = &self.lines[cursor.y];
        if cursor.x >= line.len() {
            return false; // Already at end of line
        }
        
        // Extract the text to be deleted
        let deleted_text = line[cursor.x..].to_string();
        
        // Delete to end of line
        self.lines[cursor.y].truncate(cursor.x);
        
        // Mark line as modified
        self.mark_line_modified(cursor.y);
        self.is_modified = true;
        
        // Record the action in history
        self.history.push(EditorAction {
            action_type: ActionType::DeleteToEndOfLine { 
                position: (cursor.y, cursor.x),
                deleted_text,
            },
            cursor_before,
            cursor_after,
        });
        
        true
    }
    
    /// Delete from start of line to cursor
    /// Returns true if deletion was successful
    pub fn delete_to_beginning_of_line(&mut self, cursor: &Cursor) -> bool {
        // Create the action before modifying the buffer
        let cursor_before = *cursor;
        let cursor_after = Cursor { x: 0, y: cursor.y }; // Cursor moves to start of line
        
        if cursor.y >= self.lines.len() {
            return false;
        }
        
        let line = &self.lines[cursor.y];
        if cursor.x == 0 {
            return false; // Already at start of line
        }
        
        // Extract the text to be deleted
        let deleted_text = line[0..cursor.x].to_string();
        
        // Delete from start of line to cursor
        self.lines[cursor.y] = line[cursor.x..].to_string();
        
        // Mark line as modified
        self.mark_line_modified(cursor.y);
        self.is_modified = true;
        
        // Record the action in history
        self.history.push(EditorAction {
            action_type: ActionType::DeleteToStartOfLine { 
                position: (cursor.y, cursor.x),
                deleted_text,
            },
            cursor_before,
            cursor_after,
        });
        
        true
    }
    
    // We use the similar crate for diffing, so we no longer need the load_file_for_diff method
    
    /// Find differences between current buffer and on-disk version using sophisticated diff algorithm
    pub fn diff_with_disk(&self) -> Result<HashSet<usize>> {
        // If no file path, can't diff
        let path = match &self.file_path {
            Some(p) => p,
            None => return Ok(HashSet::new()),
        };
        
        // Read the disk content
        let disk_content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        
        // Get the buffer content as a single string
        let buffer_content = self.get_content();
        
        // Create a diff
        let diff = TextDiff::from_lines(&disk_content, &buffer_content);
        
        // Track only the changed lines in the buffer
        let mut diff_lines = HashSet::new();
        
        // Process hunks of changes
        let mut buffer_line = 0; // Current line in the buffer
        
        // Process each change
        for change in diff.iter_all_changes() {
            match change.tag() {
                // Equal lines mean no change - just advance our buffer position
                ChangeTag::Equal => {
                    buffer_line += 1;
                },
                // Insertions are lines that exist in the buffer but not on disk
                ChangeTag::Insert => {
                    // Mark this line as changed
                    diff_lines.insert(buffer_line);
                    buffer_line += 1;
                },
                // Deletions are lines that exist on disk but not in buffer
                // We don't increment buffer_line since these lines don't exist in the buffer
                ChangeTag::Delete => {}
            }
        }
        
        // Add context lines for better visual feedback - 
        // only 1 line of context above/below to avoid excessive highlighting
        let context_lines = diff_lines.clone();
        for line in context_lines {
            // One line above (if possible)
            if line > 0 {
                diff_lines.insert(line - 1);
            }
            
            // One line below (if possible)
            if line + 1 < self.lines.len() {
                diff_lines.insert(line + 1);
            }
        }
        
        Ok(diff_lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_delete_line_with_undo() {
        let mut buffer = Buffer::new();
        buffer.lines = vec![
            "First line".to_string(),
            "Second line".to_string(),
            "Third line".to_string(),
        ];
        
        let mut cursor = Cursor { x: 0, y: 0 };
        
        // Delete the second line
        buffer.delete_line(1);
        
        // Check that the line was deleted
        assert_eq!(buffer.lines.len(), 2);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "Third line");
        
        // Check if we can undo
        assert!(buffer.history.can_undo());
        
        // Undo the deletion
        let undo_successful = buffer.undo(&mut cursor);
        assert!(undo_successful);
        
        // Check that the line was restored
        assert_eq!(buffer.lines.len(), 3);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "Second line");
        assert_eq!(buffer.lines[2], "Third line");
    }
    
    #[test]
    fn test_open_line_below() {
        let mut buffer = Buffer::new();
        buffer.lines = vec![
            "First line".to_string(),
            "Second line".to_string(),
        ];
        
        // Open a line below index 0 (after "First line")
        let new_line_idx = buffer.open_line_below(0);
        
        // Verify the new line is inserted
        assert_eq!(new_line_idx, 1);
        assert_eq!(buffer.lines.len(), 3);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "");
        assert_eq!(buffer.lines[2], "Second line");
        
        // Verify the line is marked as modified
        assert!(buffer.is_line_modified(1));
        
        // Test undo/redo
        let mut cursor = Cursor { x: 0, y: 0 };
        assert!(buffer.undo(&mut cursor));
        
        // Verify the line was removed during undo
        assert_eq!(buffer.lines.len(), 2);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "Second line");
        
        // Redo the operation
        assert!(buffer.redo(&mut cursor));
        
        // Verify the line was added back during redo
        assert_eq!(buffer.lines.len(), 3);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "");
        assert_eq!(buffer.lines[2], "Second line");
    }
    
    #[test]
    fn test_open_line_above() {
        let mut buffer = Buffer::new();
        buffer.lines = vec![
            "First line".to_string(),
            "Second line".to_string(),
        ];
        
        // Open a line above index 1 (before "Second line")
        let new_line_idx = buffer.open_line_above(1);
        
        // Verify the new line is inserted
        assert_eq!(new_line_idx, 1);
        assert_eq!(buffer.lines.len(), 3);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "");
        assert_eq!(buffer.lines[2], "Second line");
        
        // Verify the line is marked as modified
        assert!(buffer.is_line_modified(1));
        
        // Test undo/redo
        let mut cursor = Cursor { x: 0, y: 0 };
        assert!(buffer.undo(&mut cursor));
        
        // Verify the line was removed during undo
        assert_eq!(buffer.lines.len(), 2);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "Second line");
        
        // Redo the operation
        assert!(buffer.redo(&mut cursor));
        
        // Verify the line was added back during redo
        assert_eq!(buffer.lines.len(), 3);
        assert_eq!(buffer.lines[0], "First line");
        assert_eq!(buffer.lines[1], "");
        assert_eq!(buffer.lines[2], "Second line");
    }
}