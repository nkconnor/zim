use anyhow::{Context, Result};
use std::fs;
use std::collections::HashSet;
use super::cursor::Cursor;
use similar::{ChangeTag, TextDiff};
use syntect::parsing::SyntaxReference;
use std::sync::Arc;

pub struct Buffer {
    pub lines: Vec<String>,
    pub file_path: Option<String>,
    pub modified_lines: HashSet<usize>,
    pub is_modified: bool,
    pub syntax: Option<Arc<SyntaxReference>>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            file_path: None,
            modified_lines: HashSet::new(),
            is_modified: false,
            syntax: None,
        }
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
    }

    pub fn delete_char_at_cursor(&mut self, cursor: &Cursor) {
        if cursor.y < self.lines.len() {
            let line = &mut self.lines[cursor.y];
            if cursor.x < line.len() {
                line.remove(cursor.x);
                // Mark line as modified
                self.modified_lines.insert(cursor.y);
                self.is_modified = true;
            }
        }
    }
    
    pub fn delete_line(&mut self, line_idx: usize) {
        if line_idx < self.lines.len() {
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
        }
    }
    
    pub fn join_line(&mut self, line_idx: usize) {
        // Join current line with the next line
        if line_idx < self.lines.len() - 1 {
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
        }
    }

    pub fn insert_newline_at_cursor(&mut self, cursor: &Cursor) {
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

        if cursor.x < line.len() {
            line.truncate(cursor.x);
        }

        self.lines.insert(cursor.y + 1, new_line);
        
        // Mark both lines as modified
        self.modified_lines.insert(cursor.y);
        self.modified_lines.insert(cursor.y + 1);
        self.is_modified = true;
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
    
    /// Get all modified line indices
    pub fn get_modified_lines(&self) -> &HashSet<usize> {
        &self.modified_lines
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