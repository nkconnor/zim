use anyhow::{Context, Result};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use walkdir::WalkDir;

pub struct FileFinder {
    query: String,
    files: Vec<String>,
    matches: Vec<(String, i64)>,
    selected_index: usize,
    matcher: SkimMatcherV2,
}

impl FileFinder {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            files: Vec::new(),
            matches: Vec::new(),
            selected_index: 0,
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.query.clear();
        self.files.clear();
        self.matches.clear();
        self.selected_index = 0;

        // Get current directory
        let current_dir = std::env::current_dir()
            .context("Failed to get current directory")?;

        // Scan for files (ignoring .git and other common ignore patterns)
        for entry in WalkDir::new(&current_dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                !e.path()
                    .components()
                    .any(|c| {
                        let c = c.as_os_str().to_string_lossy();
                        c == ".git" || c == "target" || c.starts_with('.')
                    })
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
        {
            if let Some(path) = entry.path().strip_prefix(&current_dir).ok() {
                if let Some(path_str) = path.to_str() {
                    self.files.push(path_str.to_string());
                }
            }
        }

        // Sort files alphabetically for initial display
        self.files.sort();
        
        // Initialize matches with all files when query is empty
        self.matches = self.files.iter()
            .map(|f| (f.clone(), 0))
            .collect();

        Ok(())
    }

    pub fn update_matches(&mut self) -> Result<()> {
        self.selected_index = 0;
        
        if self.query.is_empty() {
            // If query is empty, show all files sorted alphabetically
            self.matches = self.files.iter()
                .map(|f| (f.clone(), 0))
                .collect();
            return Ok(());
        }

        // Filter files based on fuzzy matching
        self.matches = self.files.iter()
            .filter_map(|file| {
                self.matcher
                    .fuzzy_match(file, &self.query)
                    .map(|score| (file.clone(), score))
            })
            .collect();

        // Sort by match score, higher scores first
        self.matches.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(())
    }

    pub fn get_selected(&self) -> Option<String> {
        self.matches.get(self.selected_index).map(|(path, _)| path.clone())
    }

    pub fn add_char(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn remove_char(&mut self) {
        self.query.pop();
    }

    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.matches.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn matches(&self) -> &[(String, i64)] {
        &self.matches
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_finder_basic_operations() {
        let mut finder = FileFinder::new();
        
        // Initial state should be empty
        assert_eq!(finder.query(), "");
        assert_eq!(finder.matches().len(), 0);
        assert_eq!(finder.selected_index(), 0);
        
        // Adding and removing characters should work
        finder.add_char('t');
        finder.add_char('e');
        finder.add_char('s');
        finder.add_char('t');
        assert_eq!(finder.query(), "test");
        
        finder.remove_char();
        assert_eq!(finder.query(), "tes");
    }

    #[test]
    fn test_file_finder_navigation() {
        let mut finder = FileFinder::new();
        
        // Manually add some test files
        finder.files = vec!["file1.rs".to_string(), "file2.rs".to_string(), "file3.rs".to_string()];
        finder.matches = finder.files.iter().map(|f| (f.clone(), 0)).collect();
        
        assert_eq!(finder.selected_index(), 0);
        
        // Test navigation
        finder.next();
        assert_eq!(finder.selected_index(), 1);
        
        finder.next();
        assert_eq!(finder.selected_index(), 2);
        
        finder.next();
        assert_eq!(finder.selected_index(), 0); // Should wrap around
        
        finder.previous();
        assert_eq!(finder.selected_index(), 2);
    }

    #[test]
    fn test_fuzzy_matching() {
        let mut finder = FileFinder::new();
        
        // Manually add some test files
        finder.files = vec![
            "src/main.rs".to_string(),
            "src/editor/mod.rs".to_string(),
            "src/editor/buffer.rs".to_string(),
            "src/config/mod.rs".to_string(),
        ];
        
        // Initial matches should contain all files
        finder.update_matches().unwrap();
        assert_eq!(finder.matches().len(), 4);
        
        // Add a query and update matches
        finder.add_char('e');
        finder.add_char('d');
        finder.update_matches().unwrap();
        
        // Should match files with "ed" in them
        let matches: Vec<&String> = finder.matches().iter().map(|(path, _)| path).collect();
        assert!(matches.contains(&&"src/editor/mod.rs".to_string()));
        assert!(matches.contains(&&"src/editor/buffer.rs".to_string()));
    }
}