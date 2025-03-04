use std::collections::HashMap;

/// Severity level of a diagnostic
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// Represents a span of text in the editor (line and column range)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSpan {
    pub line: usize,
    pub start_column: usize,
    pub end_column: usize,
}

impl TextSpan {
    pub fn new(line: usize, start_column: usize, end_column: usize) -> Self {
        Self {
            line,
            start_column,
            end_column,
        }
    }
    
    /// Check if a position is within this span
    pub fn contains(&self, line: usize, column: usize) -> bool {
        line == self.line && column >= self.start_column && column < self.end_column
    }
}

/// A diagnostic message for the editor
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub span: TextSpan,
    pub related_info: Option<String>,
}

impl Diagnostic {
    pub fn new(message: &str, severity: DiagnosticSeverity, span: TextSpan) -> Self {
        Self {
            message: message.to_string(),
            severity,
            span,
            related_info: None,
        }
    }
    
    pub fn with_related_info(mut self, info: &str) -> Self {
        self.related_info = Some(info.to_string());
        self
    }
}

/// Collection of diagnostics for the current file
#[derive(Debug, Clone, Default)]
pub struct DiagnosticCollection {
    // Mapping from line number to the diagnostics on that line
    diagnostics_by_line: HashMap<usize, Vec<Diagnostic>>,
}

impl DiagnosticCollection {
    pub fn new() -> Self {
        Self {
            diagnostics_by_line: HashMap::new(),
        }
    }
    
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        let line = diagnostic.span.line;
        self.diagnostics_by_line
            .entry(line)
            .or_insert_with(Vec::new)
            .push(diagnostic);
    }
    
    pub fn clear(&mut self) {
        self.diagnostics_by_line.clear();
    }
    
    pub fn get_diagnostics_for_line(&self, line: usize) -> Option<&Vec<Diagnostic>> {
        self.diagnostics_by_line.get(&line)
    }
    
    pub fn get_all_diagnostics(&self) -> Vec<&Diagnostic> {
        self.diagnostics_by_line
            .values()
            .flat_map(|diags| diags.iter())
            .collect()
    }
    
    /// Parse diagnostics from cargo output
    pub fn parse_cargo_output(&mut self, output: &str, file_path: &str) {
        self.clear();
        
        // Simple parser for common cargo diagnostic format
        for line in output.lines() {
            // Example: warning: unused import: `sql::Pool`
            //   --> crates/admin/src/debug.rs:16:5
            if line.trim().is_empty() {
                continue;
            }
            
            // Check if the line contains a diagnostic
            if let Some(error_idx) = line.find("error:") {
                self.parse_diagnostic(line, error_idx, DiagnosticSeverity::Error, file_path);
            } else if let Some(warning_idx) = line.find("warning:") {
                self.parse_diagnostic(line, warning_idx, DiagnosticSeverity::Warning, file_path);
            }
        }
    }
    
    fn parse_diagnostic(&mut self, line: &str, idx: usize, severity: DiagnosticSeverity, file_path: &str) {
        // Extract message
        let message_start = idx + "error:".len();
        let message = line[message_start..].trim().to_string();
        
        // Look for the next line with file location
        // TODO: Implement proper parsing of the location from next lines
        // For now, we'll just add a generic diagnostic at the start of the file
        
        let diagnostic = Diagnostic::new(
            &message,
            severity,
            TextSpan::new(0, 0, 10), // Default span at start of file
        );
        
        self.add_diagnostic(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_diagnostic_creation() {
        let span = TextSpan::new(10, 5, 15);
        let diagnostic = Diagnostic::new("Unused variable", DiagnosticSeverity::Warning, span);
        
        assert_eq!(diagnostic.message, "Unused variable");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostic.span.line, 10);
        assert_eq!(diagnostic.span.start_column, 5);
        assert_eq!(diagnostic.span.end_column, 15);
        assert_eq!(diagnostic.related_info, None);
    }
    
    #[test]
    fn test_diagnostic_collection() {
        let mut collection = DiagnosticCollection::new();
        
        // Add diagnostic
        let diagnostic1 = Diagnostic::new(
            "Unused variable", 
            DiagnosticSeverity::Warning,
            TextSpan::new(10, 5, 15)
        );
        
        collection.add_diagnostic(diagnostic1);
        
        // Check retrieval
        let line_diagnostics = collection.get_diagnostics_for_line(10).unwrap();
        assert_eq!(line_diagnostics.len(), 1);
        assert_eq!(line_diagnostics[0].message, "Unused variable");
        
        // Add another diagnostic on the same line
        let diagnostic2 = Diagnostic::new(
            "Missing semicolon", 
            DiagnosticSeverity::Error,
            TextSpan::new(10, 20, 25)
        );
        
        collection.add_diagnostic(diagnostic2);
        
        // Check retrieval again
        let line_diagnostics = collection.get_diagnostics_for_line(10).unwrap();
        assert_eq!(line_diagnostics.len(), 2);
        
        // Clear collection
        collection.clear();
        assert!(collection.get_diagnostics_for_line(10).is_none());
    }
}