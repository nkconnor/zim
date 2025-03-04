use anyhow::{Context, Result};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use walkdir::WalkDir;
use std::collections::VecDeque;
use std::path::Path;

const MAX_RECENT_FILES: usize = 10;

pub struct FileFinder {
    query: String,
    files: Vec<String>,
    matches: Vec<(String, i64)>,
    selected_index: usize,
    matcher: SkimMatcherV2,
    recent_files: VecDeque<String>,
}

impl FileFinder {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            files: Vec::new(),
            matches: Vec::new(),
            selected_index: 0,
            matcher: SkimMatcherV2::default(),
            recent_files: VecDeque::with_capacity(MAX_RECENT_FILES),
        }
    }
    
    /// Add a file to the recent files list
    pub fn add_recent_file(&mut self, file_path: &str) {
        // Remove the file if it's already in the list to avoid duplicates
        self.recent_files.retain(|path| path != file_path);
        
        // Add the file to the front of the list (most recent)
        self.recent_files.push_front(file_path.to_string());
        
        // Keep only the most recent MAX_RECENT_FILES
        while self.recent_files.len() > MAX_RECENT_FILES {
            self.recent_files.pop_back();
        }
        
        // Update matches if we're showing recent files (empty query)
        if self.query.is_empty() {
            let _ = self.update_matches();
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
        self.matches.clear();
        
        if self.query.is_empty() {
            // If query is empty, show recent files first, then all files
            
            // First, add recent files
            for recent_file in &self.recent_files {
                // Skip files that no longer exist
                if Path::new(recent_file).exists() {
                    // Give recent files a high score for sorting
                    self.matches.push((recent_file.clone(), 1000));
                }
            }
            
            // Then add regular files that aren't in the recent list
            // Use a higher score for files in the current directory (shorter paths)
            for file in &self.files {
                if !self.recent_files.contains(file) {
                    // Score inversely proportional to path length to prioritize files in current dir
                    let base_score = 500 - file.len().min(500);
                    self.matches.push((file.clone(), base_score as i64));
                }
            }
            
            // Sort by score
            self.matches.sort_by(|a, b| b.1.cmp(&a.1));
            
            return Ok(());
        }

        // Filter files based on fuzzy matching
        for file in &self.files {
            if let Some(score) = self.matcher.fuzzy_match(file, &self.query) {
                // Boost score for recent files
                let boosted_score = if self.recent_files.contains(file) {
                    score + 1000 // Substantially boost recent files
                } else {
                    score
                };
                
                self.matches.push((file.clone(), boosted_score));
            }
        }

        // Sort by match score, higher scores first
        self.matches.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Limit to 100 results for performance
        if self.matches.len() > 100 {
            self.matches.truncate(100);
        }

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
 

        // just a comment      
        // Should match files with "ed" in them
        let matches: Vec<&String> = finder.matches().iter().map(|(path, _)| path).collect();
        assert!(matches.contains(&&"src/editor/mod.rs".to_string()));
        assert!(matches.contains(&&"src/editor/buffer.rs".to_string()));
    }
}