use super::Buffer;

#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub x: usize, // Column
    pub y: usize, // Row
}

impl Cursor {
    pub fn new() -> Self {
        Self { x: 0, y: 0 }
    }

    pub fn move_left(&mut self, _buffer: &Buffer) {
        if self.x > 0 {
            self.x -= 1;
        }
    }

    pub fn move_right(&mut self, buffer: &Buffer) {
        let line_length = buffer.line_length(self.y);
        if self.x < line_length {
            self.x += 1;
        }
    }

    pub fn move_up(&mut self, buffer: &Buffer) {
        if self.y > 0 {
            self.y -= 1;
            // Adjust x if new line is shorter
            let line_length = buffer.line_length(self.y);
            if self.x > line_length {
                self.x = line_length;
            }
        }
    }

    pub fn move_down(&mut self, buffer: &Buffer) {
        if self.y < buffer.line_count() - 1 {
            self.y += 1;
            // Adjust x if new line is shorter
            let line_length = buffer.line_length(self.y);
            if self.x > line_length {
                self.x = line_length;
            }
        }
    }

    pub fn move_to_line_start(&mut self, _buffer: &Buffer) {
        self.x = 0;
    }

    pub fn move_to_line_end(&mut self, buffer: &Buffer) {
        self.x = buffer.line_length(self.y);
    }

    pub fn move_to_file_start(&mut self, _buffer: &Buffer) {
        self.x = 0;
        self.y = 0;
    }

    pub fn move_to_file_end(&mut self, buffer: &Buffer) {
        self.y = buffer.line_count().saturating_sub(1);
        self.x = buffer.line_length(self.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Buffer;

    // Helper to create a buffer with test content
    fn create_test_buffer() -> Buffer {
        let mut buffer = Buffer::new();
        buffer.lines = vec![
            "First line".to_string(),
            "Second, longer line".to_string(),
            "Third line".to_string(),
            "Fourth".to_string(),
        ];
        buffer
    }

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new();
        assert_eq!(cursor.x, 0);
        assert_eq!(cursor.y, 0);
    }

    #[test]
    fn test_move_left() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        cursor.x = 5;
        
        // Normal movement
        cursor.move_left(&buffer);
        assert_eq!(cursor.x, 4);
        
        // Hit left boundary
        cursor.x = 0;
        cursor.move_left(&buffer);
        assert_eq!(cursor.x, 0);
    }

    #[test]
    fn test_move_right() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        
        // Normal movement
        cursor.move_right(&buffer);
        assert_eq!(cursor.x, 1);
        
        // Hit right boundary
        cursor.x = 10; // "First line" length is 10
        cursor.move_right(&buffer);
        assert_eq!(cursor.x, 10);
    }

    #[test]
    fn test_move_up() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        cursor.y = 2; // "Third line"
        cursor.x = 5;
        
        // Normal movement
        cursor.move_up(&buffer);
        assert_eq!(cursor.y, 1); // "Second, longer line"
        assert_eq!(cursor.x, 5);
        
        // Hit top boundary
        cursor.y = 0;
        cursor.move_up(&buffer);
        assert_eq!(cursor.y, 0);
        
        // Test x adjustment when moving to shorter line
        cursor.y = 2; // "Third line"
        cursor.x = 15;
        cursor.move_up(&buffer);
        assert_eq!(cursor.y, 1);
        assert_eq!(cursor.x, 15); // no adjustment needed
        
        cursor.move_up(&buffer);
        assert_eq!(cursor.y, 0);
        assert_eq!(cursor.x, 10); // adjusted to "First line" length
    }

    #[test]
    fn test_move_down() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        
        // Normal movement
        cursor.move_down(&buffer);
        assert_eq!(cursor.y, 1);
        
        // Hit bottom boundary
        cursor.y = 3;
        cursor.move_down(&buffer);
        assert_eq!(cursor.y, 3);
        
        // Test x adjustment when moving to shorter line
        cursor.y = 1; // "Second, longer line"
        cursor.x = 15;
        cursor.move_down(&buffer);
        assert_eq!(cursor.y, 2);
        assert_eq!(cursor.x, 10); // adjusted to "Third line" length
    }

    #[test]
    fn test_move_to_line_start() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        cursor.x = 5;
        
        cursor.move_to_line_start(&buffer);
        assert_eq!(cursor.x, 0);
    }

    #[test]
    fn test_move_to_line_end() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        
        // First line (length 10)
        cursor.move_to_line_end(&buffer);
        assert_eq!(cursor.x, 10);
        
        // Second line (length 19)
        cursor.y = 1;
        cursor.move_to_line_end(&buffer);
        assert_eq!(cursor.x, 19);
    }

    #[test]
    fn test_move_to_file_start() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        cursor.x = 5;
        cursor.y = 2;
        
        cursor.move_to_file_start(&buffer);
        assert_eq!(cursor.x, 0);
        assert_eq!(cursor.y, 0);
    }

    #[test]
    fn test_move_to_file_end() {
        let buffer = create_test_buffer();
        let mut cursor = Cursor::new();
        
        cursor.move_to_file_end(&buffer);
        assert_eq!(cursor.y, 3); // Last line index
        assert_eq!(cursor.x, 6); // "Fourth" length
    }
}