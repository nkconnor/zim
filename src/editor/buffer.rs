use anyhow::{Context, Result};
use std::fs;
use std::collections::HashSet;
use super::cursor::Cursor;

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
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        
        self.lines = content.lines().map(String::from).collect();
        
        // Ensure we have at least one line
        if self.lines.is_empty() {
            self.lines.push(String::new());
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
        self.lines.join("\n")
    }
    
    /// Set the entire content of the buffer from a string
    pub fn set_content(&mut self, content: &str) -> Result<()> {
        self.lines = content.lines().map(String::from).collect();
        
        // Ensure we have at least one line
        if self.lines.is_empty() {
            self.lines.push(String::new());
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
    
    /// Load file content for comparison without applying it to the buffer
    pub fn load_file_for_diff(&self, path: &str) -> Result<Vec<String>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        
        let lines = content.lines().map(String::from).collect();
        
        Ok(lines)
    }
    
    /// Find differences between current buffer and on-disk version
    pub fn diff_with_disk(&self) -> Result<HashSet<usize>> {
        // If no file path, can't diff
        let path = match &self.file_path {
            Some(p) => p,
            None => return Ok(HashSet::new()),
        };
        
        // Load the on-disk content
        let on_disk_lines = self.load_file_for_diff(path)?;
        
        // Find differences
        let mut diff_lines = HashSet::new();
        
        // The maximum length of the two line sets
        let max_lines = self.lines.len().max(on_disk_lines.len());
        
        for i in 0..max_lines {
            // Line exists in both buffer and on disk
            if i < self.lines.len() && i < on_disk_lines.len() {
                if self.lines[i] != on_disk_lines[i] {
                    diff_lines.insert(i);
                }
            } 
            // Line exists in buffer but not on disk
            else if i < self.lines.len() {
                diff_lines.insert(i);
            }
            // Line exists on disk but not in buffer
            else if i < on_disk_lines.len() {
                diff_lines.insert(i);
            }
        }
        
        Ok(diff_lines)
    }
}