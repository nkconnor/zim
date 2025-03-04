# Zim Editor

A modern, fast terminal-based text editor with Vim-like keybindings, syntax highlighting, and diagnostics.

![Zim Editor Screenshot](https://example.com/zim-screenshot.png)

## Features

- **Modal Editing** - Vim-like modal editing with Normal, Insert, Visual, and Command modes
- **Syntax Highlighting** - Automatic language detection with rich highlighting
- **Multiple Files** - Tab-based interface for managing multiple open files
- **Fuzzy File Finding** - Quick navigation to files using fuzzy matching
- **Code Search** - Global token search across your entire project
- **Diagnostics Panel** - View errors and warnings with contextual detail
- **Cargo Integration** - Built-in integration with Cargo for Rust projects
- **Visual Selections** - Select and manipulate text in character and line modes
- **Configurable** - Customize keybindings to match your preferences
- **Live Diffing** - See exactly what changed when reloading files

## Installation

### From Source

```bash
git clone https://github.com/yourusername/zim.git
cd zim
cargo build --release
```

The binary will be available at `target/release/zim`.

## Usage

```bash
zim [file]        # Open a file or start with the file finder
```

## Quick Start Guide

1. **Opening Files**: Use Ctrl+o to open the file finder, then type to search
2. **Moving Around**: Use h, j, k, l to navigate text (just like in Vim)
3. **Editing Text**: Press i to enter Insert mode, ESC to return to Normal mode
4. **Saving Changes**: Press w in Normal mode to save the current file
5. **Searching Code**: Use Ctrl+t to search for tokens across your project
6. **Viewing Help**: Press Ctrl+h to view all available commands
7. **Managing Tabs**: Use Ctrl+n for a new tab, Ctrl+w to close, F1-F12 for direct access

## Key Commands

### Mode Switching
- `ESC` - Return to Normal mode (from any mode)
- `i` - Enter Insert mode
- `:` - Enter Command mode
- `v` - Enter Visual mode
- `V` - Enter Visual Line mode

### File Operations
- `Ctrl+o` - Open file finder
- `w` - Save current file
- `e` - Reload file from disk
- `q` - Quit editor
- `:q!` - Force quit (discard changes)
- `X` or `ZZ` - Save and quit

### Navigation
- `h, j, k, l` - Move left, down, up, right
- `^` - Move to start of line
- `$` - Move to end of line
- `g` - Move to top of file
- `G` - Move to bottom of file
- `Ctrl+b` - Page up
- `Ctrl+f` - Page down

### Tab Management
- `Ctrl+n` - New tab
- `Ctrl+w` - Close current tab
- `Ctrl+right/left` - Next/Previous tab
- `F1-F12` - Switch directly to tabs 1-12

### Search & Diagnostics
- `Ctrl+t` - Search for code tokens across files
- `Ctrl+e` - Open diagnostics panel
- `n/p` - Navigate to next/previous diagnostic

### Rust Integration
- `Ctrl+d` - Run cargo check and show diagnostics
- `Ctrl+y` - Run cargo clippy and show diagnostics

## Configuration

Zim reads configuration from the following locations:
- Linux/macOS: `~/.config/zim/config.toml`
- Windows: `%APPDATA%\zim\config.toml`

Example configuration:

```toml
[editor]
tab_size = 4
use_spaces = true
auto_indent = true

[ui]
theme = "dark"
```

## Keybinding customization

Create a `key_bindings.toml` file next to the config.toml:

```toml
[normal_mode]
save_file = { key = "w" }
reload_file = { key = "e" }
quit = { key = "q" }

[insert_mode]
normal_mode = { key = "esc" }
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License