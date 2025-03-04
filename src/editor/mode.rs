/// Editor modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal mode (default, for navigation)
    Normal,
    /// Insert mode (for typing text)
    Insert,
    /// Command mode (for executing commands)
    Command,
    /// File finder mode (for finding and opening files)
    FileFinder,
    /// Token search mode (for finding and navigating to code tokens)
    TokenSearch,
    /// Help mode (displays keyboard shortcuts and help information)
    Help,
    /// Write confirmation mode (for confirming file write)
    WriteConfirm,
    /// Filename prompt mode (for providing a filename when saving)
    FilenamePrompt,
    /// Reload confirmation mode (for confirming file reload)
    ReloadConfirm,
    /// Visual mode (for character-based selections)
    Visual,
    /// Visual Line mode (for line-based selections)
    VisualLine,
}