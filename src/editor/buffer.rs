use anyhow::{Context, Result};
use std::fs;
use super::cursor::Cursor;

pub struct Buffer {
    pub lines: Vec<String>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
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
        
        Ok(())
    }

    pub fn insert_char_at_cursor(&mut self, c: char, cursor: &Cursor) {
        if cursor.y >= self.lines.len() {
            // Add empty lines if cursor is beyond current lines
            while cursor.y >= self.lines.len() {
                self.lines.push(String::new());
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
    }

    pub fn delete_char_at_cursor(&mut self, cursor: &Cursor) {
        if cursor.y < self.lines.len() {
            let line = &mut self.lines[cursor.y];
            if cursor.x < line.len() {
                line.remove(cursor.x);
            }
        }
    }

    pub fn insert_newline_at_cursor(&mut self, cursor: &Cursor) {
        if cursor.y >= self.lines.len() {
            // Add empty lines if cursor is beyond current lines
            while cursor.y >= self.lines.len() {
                self.lines.push(String::new());
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
}