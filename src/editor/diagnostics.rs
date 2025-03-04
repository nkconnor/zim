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
    
    /// Parse diagnostics from cargo output, filtering to only include the current file
    pub fn parse_cargo_output(&mut self, output: &str, current_file_path: &str) {
        self.clear();
        
        // Extract the filename without the path for easier matching
        let current_filename = if let Some(filename) = std::path::Path::new(current_file_path).file_name() {
            filename.to_string_lossy().to_string()
        } else {
            // If we can't extract the filename, use the full path
            current_file_path.to_string()
        };
        
        // We need to handle multi-line cargo output
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;
        
        while i < lines.len() {
            let line = lines[i].trim();
            
            // Skip empty lines
            if line.is_empty() {
                i += 1;
                continue;
            }
            
            // Look for diagnostic pattern
            let (severity, message) = if let Some(error_idx) = line.find("error:") {
                let message_start = error_idx + "error:".len();
                (DiagnosticSeverity::Error, line[message_start..].trim().to_string())
            } else if let Some(warning_idx) = line.find("warning:") {
                let message_start = warning_idx + "warning:".len();
                (DiagnosticSeverity::Warning, line[message_start..].trim().to_string())
            } else {
                // Not a diagnostic line
                i += 1;
                continue;
            };
            
            // Try to find the location information in the next line
            let mut line_num = 0;
            let mut col_start = 0;
            let mut col_end = 10; // Default span width
            let mut is_current_file = false;
            
            // Look ahead for location line
            if i + 1 < lines.len() {
                let location_line = lines[i + 1].trim();
                // Format is typically: --> file:line:column
                if location_line.contains("-->") {
                    // Extract the file path
                    let parts: Vec<&str> = location_line.split("-->").collect();
                    if parts.len() > 1 {
                        let file_info = parts[1].trim();
                        
                        // Check if this is for the current file
                        is_current_file = file_info.contains(&current_filename);
                        
                        // Parse line and column if it's for the current file
                        if is_current_file {
                            let file_parts: Vec<&str> = file_info.split(":").collect();
                            if file_parts.len() > 1 {
                                // Parse line number
                                if let Ok(parsed_line) = file_parts[1].trim().parse::<usize>() {
                                    line_num = parsed_line.saturating_sub(1); // 0-indexed in our editor
                                }
                                
                                // Parse column
                                if file_parts.len() > 2 {
                                    if let Ok(parsed_col) = file_parts[2].trim().parse::<usize>() {
                                        col_start = parsed_col.saturating_sub(1); // 0-indexed
                                        
                                        // Try to determine the span width from the code
                                        if i + 3 < lines.len() && lines[i + 3].contains("^") {
                                            let span_line = lines[i + 3].trim_start();
                                            let span_width = span_line.chars().take_while(|&c| c == '^').count();
                                            if span_width > 0 {
                                                col_end = col_start + span_width;
                                            } else {
                                                col_end = col_start + 10; // Default
                                            }
                                        } else {
                                            col_end = col_start + 10; // Default
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Only add diagnostics for the current file
            if is_current_file {
                // Create the diagnostic
                let diagnostic = Diagnostic::new(
                    &message,
                    severity,
                    TextSpan::new(line_num, col_start, col_end),
                );
                
                self.add_diagnostic(diagnostic);
            }
            
            i += 1;
        }
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