#[derive(Clone)]
pub struct Viewport {
    pub top_line: usize,     // First visible line
    pub height: usize,       // Number of visible lines
    pub left_column: usize,  // First visible column
    pub width: usize,        // Number of visible columns
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            top_line: 0,
            height: 0,
            left_column: 0,
            width: 0,
        }
    }

    pub fn update_dimensions(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    pub fn ensure_cursor_visible(&mut self, cursor_y: usize, cursor_x: usize) {
        // Vertical scrolling
        if cursor_y < self.top_line {
            // Cursor is above viewport
            self.top_line = cursor_y;
        } else if cursor_y >= self.top_line + self.height {
            // Cursor is below viewport
            self.top_line = cursor_y - self.height + 1;
        }

        // Horizontal scrolling
        if cursor_x < self.left_column {
            // Cursor is to the left of viewport
            self.left_column = cursor_x;
        } else if cursor_x >= self.left_column + self.width {
            // Cursor is to the right of viewport
            self.left_column = cursor_x - self.width + 1;
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        if self.top_line > lines {
            self.top_line -= lines;
        } else {
            self.top_line = 0;
        }
    }

    pub fn scroll_down(&mut self, lines: usize, max_line: usize) {
        let max_top = if max_line > self.height {
            max_line - self.height + 1
        } else {
            0
        };

        if self.top_line + lines <= max_top {
            self.top_line += lines;
        } else {
            self.top_line = max_top;
        }
    }

    pub fn get_visible_range(&self, total_lines: usize) -> (usize, usize) {
        let start = self.top_line;
        let end = std::cmp::min(self.top_line + self.height, total_lines);
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_new() {
        let viewport = Viewport::new();
        assert_eq!(viewport.top_line, 0);
        assert_eq!(viewport.height, 0);
        assert_eq!(viewport.left_column, 0);
        assert_eq!(viewport.width, 0);
    }

    #[test]
    fn test_update_dimensions() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(80, 24);
        assert_eq!(viewport.width, 80);
        assert_eq!(viewport.height, 24);
        // Other properties should remain unchanged
        assert_eq!(viewport.top_line, 0);
        assert_eq!(viewport.left_column, 0);
    }

    #[test]
    fn test_ensure_cursor_visible_initial() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(80, 24);
        
        // Cursor within viewport initially
        viewport.ensure_cursor_visible(10, 10);
        assert_eq!(viewport.top_line, 0);
        assert_eq!(viewport.left_column, 0);
    }

    #[test]
    fn test_ensure_cursor_visible_below() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(10, 5);
        
        // Cursor below viewport
        viewport.ensure_cursor_visible(10, 5);
        assert_eq!(viewport.top_line, 6); // 10 - 5 + 1 = 6
        assert_eq!(viewport.left_column, 0);
    }

    #[test]
    fn test_ensure_cursor_visible_right() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(10, 5);
        
        // Cursor to the right of viewport
        viewport.ensure_cursor_visible(2, 15);
        assert_eq!(viewport.top_line, 0);
        assert_eq!(viewport.left_column, 6); // 15 - 10 + 1 = 6
    }

    #[test]
    fn test_ensure_cursor_visible_above() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(10, 5);
        viewport.top_line = 10;
        
        // Cursor above viewport
        viewport.ensure_cursor_visible(5, 5);
        assert_eq!(viewport.top_line, 5);
        assert_eq!(viewport.left_column, 0);
    }

    #[test]
    fn test_ensure_cursor_visible_left() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(10, 5);
        viewport.left_column = 10;
        
        // Cursor to the left of viewport
        viewport.ensure_cursor_visible(2, 5);
        assert_eq!(viewport.top_line, 0);
        assert_eq!(viewport.left_column, 5);
    }

    #[test]
    fn test_scroll_up() {
        let mut viewport = Viewport::new();
        viewport.top_line = 20;
        
        // Scroll up 5 lines
        viewport.scroll_up(5);
        assert_eq!(viewport.top_line, 15);
        
        // Scroll up beyond top
        viewport.scroll_up(20);
        assert_eq!(viewport.top_line, 0);
    }

    #[test]
    fn test_scroll_down() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(10, 5);
        
        // Scroll down 5 lines with plenty of content
        viewport.scroll_down(5, 100);
        assert_eq!(viewport.top_line, 5);
        
        // Scroll down to end
        viewport.scroll_down(200, 100);
        assert_eq!(viewport.top_line, 96); // max_top = 100 - 5 + 1 = 96
    }

    #[test]
    fn test_get_visible_range() {
        let mut viewport = Viewport::new();
        viewport.update_dimensions(10, 5);
        viewport.top_line = 10;
        
        // Test with plenty of content
        let (start, end) = viewport.get_visible_range(100);
        assert_eq!(start, 10);
        assert_eq!(end, 15);
        
        // Test with limited content
        let (start, end) = viewport.get_visible_range(12);
        assert_eq!(start, 10);
        assert_eq!(end, 12);
    }
}