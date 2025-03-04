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
        
        // Normalize the current file path
        let current_file = std::path::PathBuf::from(current_file_path)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(current_file_path));
        let current_filename = current_file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
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
            let (severity, message) = if let Some(error_idx) = line.find("error[") {
                // Look for the ]: that ends the error code
                if let Some(close_idx) = line[error_idx..].find("]:") {
                    let message_start = error_idx + close_idx + 2;
                    (DiagnosticSeverity::Error, line[message_start..].trim().to_string())
                } else {
                    (DiagnosticSeverity::Error, line[error_idx + 5..].trim().to_string())
                }
            } else if let Some(error_idx) = line.find("error:") {
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
                        
                        // Get file path from location line
                        let file_parts: Vec<&str> = file_info.split(':').collect();
                        if !file_parts.is_empty() {
                            let reported_file_path = file_parts[0].trim();
                            let reported_file = std::path::PathBuf::from(reported_file_path);
                            
                            // Get the filename from the reported file path
                            let reported_filename = reported_file.file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_default();
                            
                            // Try to match in various ways (from most specific to least specific)
                            is_current_file = 
                                // 1. Direct match of canonical paths (most reliable)
                                match (reported_file.canonicalize(), current_file.canonicalize()) {
                                    (Ok(a), Ok(b)) => a == b,
                                    _ => false
                                } ||
                                // 2. Direct string match of the paths
                                reported_file_path == current_file_path ||
                                // 3. If current_file_path is a suffix of reported_file_path (e.g. src/main.rs matches /home/user/project/src/main.rs)
                                reported_file_path.ends_with(current_file_path) ||
                                // 4. If current_file_path contains just a filename and it matches the reported filename
                                (std::path::Path::new(current_file_path).file_name().is_some() &&
                                 !current_file_path.contains('/') && 
                                 reported_filename == current_file_path) ||
                                // 5. If the filenames match and the paths have src/ in common
                                (reported_filename == current_filename &&
                                 (reported_file_path.ends_with(&format!("src/{}", current_filename)) ||
                                  current_file_path.ends_with(&format!("src/{}", current_filename))));
                            
                            // Parse line and column if it's for the current file
                            if is_current_file && file_parts.len() > 1 {
                                // Parse line number
                                if let Ok(parsed_line) = file_parts[1].trim().parse::<usize>() {
                                    line_num = parsed_line.saturating_sub(1); // 0-indexed in our editor
                                }
                                
                                // Parse column
                                if file_parts.len() > 2 {
                                    if let Ok(parsed_col) = file_parts[2].trim().parse::<usize>() {
                                        col_start = parsed_col.saturating_sub(1); // 0-indexed
                                        
                                        // Look ahead for the caret indicators (^^^) to determine span width
                                        let mut lookahead = 2;
                                        let mut found_caret = false;
                                        
                                        // Look up to 5 lines ahead for the carets
                                        while lookahead < 6 && i + lookahead < lines.len() {
                                            let potential_caret_line = lines[i + lookahead].trim_start();
                                            if potential_caret_line.starts_with('^') {
                                                let span_width = potential_caret_line.chars()
                                                    .take_while(|&c| c == '^')
                                                    .count();
                                                
                                                if span_width > 0 {
                                                    col_end = col_start + span_width;
                                                    found_caret = true;
                                                    break;
                                                }
                                            }
                                            lookahead += 1;
                                        }
                                        
                                        if !found_caret {
                                            // Use heuristic: for errors, use at least 5 chars; for warnings, more context-based
                                            match severity {
                                                DiagnosticSeverity::Error => col_end = col_start + 5,
                                                _ => col_end = col_start + 10
                                            }
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
                let mut diagnostic = Diagnostic::new(
                    &message,
                    severity,
                    TextSpan::new(line_num, col_start, col_end),
                );
                
                // If there are 4+ lines after this one, try to extract some context
                if i + 3 < lines.len() {
                    let context_line = lines[i + 2];
                    if !context_line.trim().is_empty() {
                        diagnostic = diagnostic.with_related_info(context_line.trim());
                    }
                }
                
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
    
    #[test]
    fn test_parse_error_diagnostic() {
        let mut collection = DiagnosticCollection::new();
        let current_file = "src/main.rs";
        
        let cargo_output = r#"
        Checking zim v0.1.0 (/home/nconnor/p/zim/zim)
        error[E0308]: mismatched types
          --> src/main.rs:52:18
           |
        52 |     let x: i32 = "not a number";
           |            ---   ^^^^^^^^^^^^^^ expected `i32`, found `&str`
           |            |
           |            expected due to this
        
        For more information about this error, try `rustc --explain E0308`.
        error: could not compile `zim` (bin "zim") due to 1 previous error
        "#;
        
        collection.parse_cargo_output(cargo_output, current_file);
        
        // Verify the diagnostic was parsed correctly
        assert_eq!(collection.get_all_diagnostics().len(), 1);
        
        let diagnostics = collection.get_diagnostics_for_line(51).unwrap(); // 0-indexed, so line 52 -> 51
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostics[0].message, "mismatched types");
        assert_eq!(diagnostics[0].span.start_column, 17); // 0-indexed
        
        // Check that the span width matches the ^^^^^^ indicators
        assert!(diagnostics[0].span.end_column > diagnostics[0].span.start_column);
    }
    
    #[test]
    fn test_parse_warning_diagnostic() {
        let mut collection = DiagnosticCollection::new();
        let current_file = "src/config/mod.rs";
        
        let cargo_output = r#"
        Checking zim v0.1.0 (/home/nconnor/p/zim/zim)
        warning: this import is redundant
         --> src/config/mod.rs:2:1
          |
        2 | use dirs;
          | ^^^^^^^^^ help: remove it entirely
          |
          = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#single_component_path_imports
          = note: `#[warn(clippy::single_component_path_imports)]` on by default
        
        warning: `zim` (bin "zim") generated 1 warning
        "#;
        
        collection.parse_cargo_output(cargo_output, current_file);
        
        // Verify the diagnostic was parsed correctly
        assert_eq!(collection.get_all_diagnostics().len(), 1);
        
        let diagnostics = collection.get_diagnostics_for_line(1).unwrap(); // 0-indexed, so line 2 -> 1
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostics[0].message, "this import is redundant");
        assert_eq!(diagnostics[0].span.start_column, 0); // 0-indexed
    }
    
    #[test]
    fn test_parse_multiple_diagnostics() {
        let mut collection = DiagnosticCollection::new();
        let current_file = "src/main.rs";
        
        let cargo_output = r#"
        Checking zim v0.1.0 (/home/nconnor/p/zim/zim)
        error[E0308]: mismatched types
          --> src/main.rs:52:18
           |
        52 |     let x: i32 = "not a number";
           |            ---   ^^^^^^^^^^^^^^ expected `i32`, found `&str`
           |            |
           |            expected due to this
        
        warning: unused variable: `x`
          --> src/main.rs:52:9
           |
        52 |     let x: i32 = "not a number";
           |         ^ help: if this is intentional, prefix it with an underscore: `_x`
           |
           = note: `#[warn(unused_variables)]` on by default
        
        error: could not compile `zim` (bin "zim") due to previous error
        "#;
        
        collection.parse_cargo_output(cargo_output, current_file);
        
        // Verify both diagnostics were parsed correctly
        assert_eq!(collection.get_all_diagnostics().len(), 2);
        
        let diagnostics = collection.get_diagnostics_for_line(51).unwrap(); // 0-indexed, so line 52 -> 51
        assert_eq!(diagnostics.len(), 2);
        
        // Find the error and warning
        let error = diagnostics.iter().find(|d| d.severity == DiagnosticSeverity::Error).unwrap();
        let warning = diagnostics.iter().find(|d| d.severity == DiagnosticSeverity::Warning).unwrap();
        
        assert_eq!(error.message, "mismatched types");
        assert_eq!(warning.message, "unused variable: `x`");
    }
    
    #[test]
    fn test_file_path_matching() {
        let mut collection = DiagnosticCollection::new();
        
        // Test with different file path formats
        let test_cases = [
            // Current file, diagnostic file, should match
            ("src/main.rs", "src/main.rs", true),
            ("/home/user/project/src/main.rs", "src/main.rs", true),
            ("src/main.rs", "/home/user/project/src/main.rs", true),
            ("main.rs", "src/main.rs", true),
            ("editor/buffer.rs", "src/editor/buffer.rs", true),
            // Different files shouldn't match
            ("src/main.rs", "src/lib.rs", false),
            ("src/editor/buffer.rs", "src/editor/cursor.rs", false),
        ];
        
        for (idx, (current_file, diagnostic_file, should_match)) in test_cases.iter().enumerate() {
            let cargo_output = format!(r#"
            Checking zim v0.1.0 (/home/user/project)
            error: test error {}
              --> {}:10:5
               |
            10 |     let x = 5;
               |     ^^^^^^^^^ test error
            "#, idx, diagnostic_file);
            
            collection.clear();
            collection.parse_cargo_output(&cargo_output, current_file);
            
            if *should_match {
                assert!(!collection.get_all_diagnostics().is_empty(), 
                    "Case {} failed: '{}' should match '{}'", idx, current_file, diagnostic_file);
            } else {
                assert!(collection.get_all_diagnostics().is_empty(),
                    "Case {} failed: '{}' should NOT match '{}'", idx, current_file, diagnostic_file);
            }
        }
    }
}