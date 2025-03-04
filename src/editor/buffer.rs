use anyhow::{Context, Result};
use std::fs;
use std::collections::HashSet;
use super::cursor::Cursor;
use similar::{ChangeTag, TextDiff};

pub struct Buffer {
    pub lines: Vec<String>,
    pub file_path: Option<String>,
    pub modified_lines: HashSet<usize>,
    pub is_modified: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            file_path: None,
            modified_lines: HashSet::new(),
            is_modified: false,
        }
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        // Read the file content as is - don't auto-split into lines
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        
        // Start fresh with no lines
        self.lines = Vec::new();
        
        // Simply split the content by newlines and handle the last line specially
        if content.is_empty() {
            // Empty file - add a single empty line
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
            
            // If the file ends with a newline, the last line should be empty
            // The loop above would have added an empty string already
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
        
        if cursor.x > line.len() {
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
    pub fn get_content(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        
        // Build the content manually with proper newline handling
        let mut content = String::new();
        
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