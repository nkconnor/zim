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
    /// Additional context such as error code, help text, or suggested fixes
    pub additional_info: Vec<String>,
    /// Source location (file path)
    pub file_path: String,
    /// Original line number from the diagnostic (1-indexed)
    pub original_line_number: usize,
}

impl Diagnostic {
    pub fn new(message: &str, severity: DiagnosticSeverity, span: TextSpan) -> Self {
        Self {
            message: message.to_string(),
            severity,
            span: span.clone(), // Clone the span to avoid move issues
            related_info: None,
            additional_info: Vec::new(),
            file_path: String::new(),
            original_line_number: span.line + 1, // Default to 1-indexed from span
        }
    }
    
    pub fn with_related_info(mut self, info: &str) -> Self {
        self.related_info = Some(info.to_string());
        self
    }
    
    pub fn with_additional_info(mut self, info: &str) -> Self {
        self.additional_info.push(info.to_string());
        self
    }
    
    pub fn with_file_path(mut self, path: &str) -> Self {
        self.file_path = path.to_string();
        self
    }
    
    pub fn with_original_line(mut self, line: usize) -> Self {
        self.original_line_number = line;
        self
    }
    
    /// Returns a formatted string with all diagnostic information
    pub fn format_full_message(&self) -> String {
        let severity_str = match self.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Information => "info",
            DiagnosticSeverity::Hint => "hint",
        };
        
        let mut output = format!("{}: {}\n", severity_str, self.message);
        
        // Add location
        if !self.file_path.is_empty() {
            output.push_str(&format!("  --> {}:{}:{}\n", 
                self.file_path, 
                self.original_line_number, 
                self.span.start_column + 1)); // 1-indexed column
        }
        
        // Add related info
        if let Some(related) = &self.related_info {
            output.push_str(&format!("  | {}\n", related));
        }
        
        // Add additional context
        for info in &self.additional_info {
            output.push_str(&format!("  = {}\n", info));
        }
        
        output
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
    
    /// Get all diagnostics filtered by severity
    pub fn get_filtered_diagnostics(&self, filter: &crate::editor::DiagnosticFilter) -> Vec<&Diagnostic> {
        match filter {
            crate::editor::DiagnosticFilter::All => {
                self.get_all_diagnostics()
            },
            crate::editor::DiagnosticFilter::Errors => {
                self.get_all_diagnostics()
                    .into_iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Error)
                    .collect()
            },
            crate::editor::DiagnosticFilter::Warnings => {
                self.get_all_diagnostics()
                    .into_iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Warning)
                    .collect()
            },
            crate::editor::DiagnosticFilter::Info => {
                self.get_all_diagnostics()
                    .into_iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Information)
                    .collect()
            },
        }
    }
    
    /// Returns the number of errors in the collection
    pub fn error_count(&self) -> usize {
        self.get_all_diagnostics()
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count()
    }
    
    /// Returns the number of warnings in the collection
    pub fn warning_count(&self) -> usize {
        self.get_all_diagnostics()
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count()
    }
    
    /// Returns a vector of all line numbers that have diagnostics
    pub fn get_diagnostic_line_numbers(&self) -> Vec<usize> {
        let mut lines: Vec<usize> = self.diagnostics_by_line.keys().cloned().collect();
        lines.sort_unstable();
        lines
    }
    
    /// Returns the next diagnostic line after the given line,
    /// or the first diagnostic line if there are none after
    pub fn next_diagnostic_line(&self, current_line: usize) -> Option<usize> {
        let lines = self.get_diagnostic_line_numbers();
        if lines.is_empty() {
            return None;
        }
        
        match lines.iter().find(|&&line| line > current_line) {
            Some(&line) => Some(line),
            None => Some(lines[0]), // Wrap around to first diagnostic
        }
    }
    
    /// Returns the previous diagnostic line before the given line,
    /// or the last diagnostic line if there are none before
    pub fn prev_diagnostic_line(&self, current_line: usize) -> Option<usize> {
        let lines = self.get_diagnostic_line_numbers();
        if lines.is_empty() {
            return None;
        }
        
        match lines.iter().rev().find(|&&line| line < current_line) {
            Some(&line) => Some(line),
            None => Some(lines[lines.len() - 1]), // Wrap around to last diagnostic
        }
    }
}

impl DiagnosticCollection {
    /// Parse diagnostics from cargo output, filtering to only include the current file
    pub fn parse_cargo_output(mut self, output: &str, current_file_path: &str) -> Self {
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
            let (severity, message, error_code) = if let Some(error_idx) = line.find("error[") {
                // Look for the ]: that ends the error code
                if let Some(close_idx) = line[error_idx..].find("]:") {
                    let code_start = error_idx + "error[".len();
                    let code_end = error_idx + close_idx;
                    let error_code = line[code_start..code_end].to_string();
                    
                    let message_start = error_idx + close_idx + 2;
                    (DiagnosticSeverity::Error, line[message_start..].trim().to_string(), Some(error_code))
                } else {
                    (DiagnosticSeverity::Error, line[error_idx + 5..].trim().to_string(), None)
                }
            } else if let Some(error_idx) = line.find("error:") {
                let message_start = error_idx + "error:".len();
                (DiagnosticSeverity::Error, line[message_start..].trim().to_string(), None)
            } else if let Some(warning_idx) = line.find("warning[") {
                // Look for the ]: that ends the warning code
                if let Some(close_idx) = line[warning_idx..].find("]:") {
                    let code_start = warning_idx + "warning[".len();
                    let code_end = warning_idx + close_idx;
                    let warning_code = line[code_start..code_end].to_string();
                    
                    let message_start = warning_idx + close_idx + 2;
                    (DiagnosticSeverity::Warning, line[message_start..].trim().to_string(), Some(warning_code))
                } else {
                    (DiagnosticSeverity::Warning, line[warning_idx + 7..].trim().to_string(), None)
                }
            } else if let Some(warning_idx) = line.find("warning:") {
                let message_start = warning_idx + "warning:".len();
                (DiagnosticSeverity::Warning, line[message_start..].trim().to_string(), None)
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
            let mut reported_file_path = String::new();
            let mut reported_line_number = 0;
            
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
                            reported_file_path = file_parts[0].trim().to_string();
                            let reported_file = std::path::PathBuf::from(&reported_file_path);
                            
                            // Get the filename from the reported file path
                            let reported_filename = reported_file.file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_default();
                            
                            // Store original line number if available
                            if file_parts.len() > 1 {
                                if let Ok(parsed_line) = file_parts[1].trim().parse::<usize>() {
                                    reported_line_number = parsed_line; // Keep 1-indexed for display
                                }
                            }
                            
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
                // Create the diagnostic with the basic information
                let mut diagnostic = Diagnostic::new(
                    &message,
                    severity,
                    TextSpan::new(line_num, col_start, col_end),
                )
                .with_file_path(&reported_file_path)
                .with_original_line(reported_line_number);
                
                // Extract code sample and surrounding context
                let mut code_context = String::new();
                let mut related_info_found = false;
                
                // Extract helpful suggestions and additional context
                let mut j = i + 2; // Start looking after the location line
                while j < i + 15 && j < lines.len() { // Look ahead up to 15 lines
                    let context_line = lines[j].trim_start();
                    
                    // Skip blank lines if we haven't found any related info yet
                    if context_line.is_empty() && !related_info_found {
                        j += 1;
                        continue;
                    }
                    
                    // Stop at the next error/warning
                    if context_line.starts_with("error") || context_line.starts_with("warning") ||
                       context_line.contains("--> ") {
                        break;
                    }
                    
                    // Capture the first line with the actual code
                    if !related_info_found && context_line.contains('|') && j + 1 < lines.len() {
                        // This is likely showing the code line
                        code_context = context_line.to_string();
                        diagnostic = diagnostic.with_related_info(&code_context);
                        related_info_found = true;
                    }
                    // Look for help or note lines starting with =
                    else if context_line.starts_with('=') || context_line.contains("help:") || context_line.contains("note:") {
                        diagnostic = diagnostic.with_additional_info(context_line);
                    }
                    // Add rust explain reference if available
                    else if context_line.contains("rustc --explain") {
                        if let Some(code) = &error_code {
                            diagnostic = diagnostic.with_additional_info(
                                &format!("For more information, run: rustc --explain {}", code)
                            );
                        }
                    }
                    
                    j += 1;
                }
                
                self.add_diagnostic(diagnostic);
            }
            
            i += 1;
        }
        
        self
    }
}

