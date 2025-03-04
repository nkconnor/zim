use anyhow::Result;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::{SyntaxSet, SyntaxReference};
use syntect::util::LinesWithEndings;
use std::sync::Arc;

/// Manages syntax highlighting
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    current_theme: String,
}

#[derive(Clone)]
pub struct HighlightedLine {
    pub ranges: Vec<(Style, String)>,
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter with default settings
    pub fn new() -> Self {
        // Load syntax definitions and themes
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        
        Self {
            syntax_set,
            theme_set,
            current_theme: "Solarized (dark)".to_string(), // Default theme
        }
    }
    
    /// Set the current theme
    pub fn set_theme(&mut self, theme_name: &str) -> Result<()> {
        if self.theme_set.themes.contains_key(theme_name) {
            self.current_theme = theme_name.to_string();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Theme not found: {}", theme_name))
        }
    }
    
    /// List available themes
    pub fn list_themes(&self) -> Vec<String> {
        self.theme_set.themes.keys().cloned().collect()
    }
    
    /// Get the current theme
    pub fn current_theme(&self) -> &str {
        &self.current_theme
    }
    
    /// Determine the syntax to use based on file extension or first line
    pub fn determine_syntax(&self, file_path: Option<&str>, first_line: &str) -> Option<Arc<SyntaxReference>> {
        // Try to determine by file path first
        if let Some(path) = file_path {
            let path = Path::new(path);
            if let Some(extension) = path.extension() {
                if let Some(extension_str) = extension.to_str() {
                    // Try to find syntax by extension
                    if let Some(syntax) = self.syntax_set.find_syntax_by_extension(extension_str) {
                        return Some(Arc::new(syntax.clone()));
                    }
                }
            }
            
            // Try by filename (use path string matching)
            let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            for syntax in self.syntax_set.syntaxes() {
                // Check file extensions
                for ext in &syntax.file_extensions {
                    if filename.ends_with(ext) {
                        return Some(Arc::new(syntax.clone()));
                    }
                }
            }
        }
        
        // Try first line detection as fallback
        if let Some(syntax) = self.syntax_set.find_syntax_by_first_line(first_line) {
            return Some(Arc::new(syntax.clone()));
        }
        
        None
    }
    
    /// Highlight a portion of text with the given syntax
    pub fn highlight_text(&self, text: &str, syntax: Arc<SyntaxReference>) -> Vec<HighlightedLine> {
        // Get the current theme
        let theme = &self.theme_set.themes[&self.current_theme];
        
        // Create a highlighter
        let mut highlighter = HighlightLines::new(&syntax, theme);
        
        // Process the text line by line
        let mut result = Vec::new();
        
        for line in LinesWithEndings::from(text) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    // Convert &str to String in ranges
                    let string_ranges = ranges.into_iter()
                        .map(|(style, text)| (style, text.to_string()))
                        .collect();
                    
                    result.push(HighlightedLine { ranges: string_ranges });
                },
                Err(_) => {
                    // If highlighting fails, add the line as plain text
                    let style = Style::default();
                    result.push(HighlightedLine {
                        ranges: vec![(style, line.to_string())]
                    });
                }
            }
        }
        
        result
    }
    
    /// Get a list of supported languages
    pub fn supported_languages(&self) -> Vec<String> {
        self.syntax_set.syntaxes().iter()
            .map(|syntax| syntax.name.clone())
            .collect()
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}