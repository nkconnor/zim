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
    /// Help mode (displays keyboard shortcuts and help information)
    Help,
    /// Write confirmation mode (for confirming file write)
    WriteConfirm,
}