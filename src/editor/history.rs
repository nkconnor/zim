use super::cursor::Cursor;

/// Represents a single undoable editor action
#[derive(Debug, Clone)]
pub enum ActionType {
    InsertChar {
        x: usize,
        y: usize,
        c: char,
    },
    DeleteChar {
        x: usize,
        y: usize,
        c: char,
    },
    InsertNewline {
        x: usize,
        y: usize,
        remaining_text: String,
    },
    DeleteLine {
        y: usize,
        content: String,
    },
    JoinLines {
        y: usize,
        column_pos: usize,
    },
    ReplaceSelection {
        old_text: String,
        new_text: String,
        selection_start: (usize, usize),
        selection_end: (usize, usize),
    },
    SetContent {
        old_lines: Vec<String>,
        new_lines: Vec<String>,
    },
    OpenLineBelow {
        y: usize,
    },
    OpenLineAbove {
        y: usize,
    },
    // New delete operations for DeleteMode
    DeleteWord {
        position: (usize, usize),  // (line, column)
        deleted_text: String,
    },
    DeleteToEndOfLine {
        position: (usize, usize),  // (line, column)
        deleted_text: String,
    },
    DeleteToStartOfLine {
        position: (usize, usize),  // (line, column)
        deleted_text: String,
    },
}

#[derive(Debug, Clone)]
pub struct EditorAction {
    pub action_type: ActionType,
    pub cursor_before: Cursor,
    pub cursor_after: Cursor,
}

/// History manager for tracking editor changes
pub struct History {
    actions: Vec<EditorAction>,
    current_index: usize,
    max_history: usize,
    /// Flag to indicate if we're currently in an undo operation
    in_undo_or_redo: bool,
}

impl History {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            current_index: 0,
            max_history: 1000, // Configurable limit
            in_undo_or_redo: false,
        }
    }

    /// Add an action to the history
    pub fn push(&mut self, action: EditorAction) {
        // Don't record actions that happen during an undo/redo operation
        if self.in_undo_or_redo {
            return;
        }

        // If we're not at the end of the history, truncate the future actions
        if self.current_index < self.actions.len() {
            self.actions.truncate(self.current_index);
        }

        // Add the new action
        self.actions.push(action);
        self.current_index = self.actions.len();

        // Trim history if it exceeds the maximum size
        if self.actions.len() > self.max_history {
            let remove_count = self.actions.len() - self.max_history;
            self.actions.drain(0..remove_count);
            self.current_index -= remove_count;
        }
    }

    /// Can we undo?
    pub fn can_undo(&self) -> bool {
        self.current_index > 0
    }

    /// Can we redo?
    pub fn can_redo(&self) -> bool {
        self.current_index < self.actions.len()
    }

    /// Get the action to undo
    pub fn undo_action(&mut self) -> Option<EditorAction> {
        if !self.can_undo() {
            return None;
        }

        self.current_index -= 1;
        let action = self.actions[self.current_index].clone();
        Some(action)
    }

    /// Get the action to redo
    pub fn redo_action(&mut self) -> Option<EditorAction> {
        if !self.can_redo() {
            return None;
        }

        let action = self.actions[self.current_index].clone();
        self.current_index += 1;
        Some(action)
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.actions.clear();
        self.current_index = 0;
    }
    
    /// Mark that we're entering an undo/redo operation
    pub fn start_undo_or_redo(&mut self) {
        self.in_undo_or_redo = true;
    }
    
    /// Mark that we're exiting an undo/redo operation
    pub fn end_undo_or_redo(&mut self) {
        self.in_undo_or_redo = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_history_push() {
        let mut history = History::new();
        
        let action = EditorAction {
            action_type: ActionType::InsertChar { x: 0, y: 0, c: 'a' },
            cursor_before: Cursor { x: 0, y: 0 },
            cursor_after: Cursor { x: 1, y: 0 },
        };
        
        history.push(action);
        
        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.current_index, 1);
    }
    
    #[test]
    fn test_history_undo_redo() {
        let mut history = History::new();
        
        let action1 = EditorAction {
            action_type: ActionType::InsertChar { x: 0, y: 0, c: 'a' },
            cursor_before: Cursor { x: 0, y: 0 },
            cursor_after: Cursor { x: 1, y: 0 },
        };
        
        let action2 = EditorAction {
            action_type: ActionType::InsertChar { x: 1, y: 0, c: 'b' },
            cursor_before: Cursor { x: 1, y: 0 },
            cursor_after: Cursor { x: 2, y: 0 },
        };
        
        history.push(action1.clone());
        history.push(action2.clone());
        
        assert!(history.can_undo());
        assert!(!history.can_redo());
        
        // Undo the second action
        let undo_action = history.undo_action().unwrap();
        match undo_action.action_type {
            ActionType::InsertChar { x, y, c } => {
                assert_eq!(x, 1);
                assert_eq!(y, 0);
                assert_eq!(c, 'b');
            },
            _ => panic!("Unexpected action type"),
        }
        
        assert!(history.can_undo());
        assert!(history.can_redo());
        
        // Redo the second action
        let redo_action = history.redo_action().unwrap();
        match redo_action.action_type {
            ActionType::InsertChar { x, y, c } => {
                assert_eq!(x, 1);
                assert_eq!(y, 0);
                assert_eq!(c, 'b');
            },
            _ => panic!("Unexpected action type"),
        }
        
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }
    
    #[test]
    fn test_history_truncation() {
        let mut history = History::new();
        
        // Add three actions
        for i in 0..3 {
            let action = EditorAction {
                action_type: ActionType::InsertChar { x: i, y: 0, c: (b'a' + i as u8) as char },
                cursor_before: Cursor { x: i, y: 0 },
                cursor_after: Cursor { x: i + 1, y: 0 },
            };
            history.push(action);
        }
        
        // Undo back to the first action
        history.undo_action();
        history.undo_action();
        
        assert_eq!(history.current_index, 1);
        
        // Add a new action, which should truncate the future
        let new_action = EditorAction {
            action_type: ActionType::InsertChar { x: 1, y: 0, c: 'x' },
            cursor_before: Cursor { x: 1, y: 0 },
            cursor_after: Cursor { x: 2, y: 0 },
        };
        history.push(new_action);
        
        // Now we should have only 2 actions
        assert_eq!(history.actions.len(), 2);
        assert!(!history.can_redo());
    }
}