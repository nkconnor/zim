# Zim Editor

A modern, fast terminal-based text editor with vim-like keybindings and intelligent features.

## Overview

Zim is a lightweight, terminal-based text editor built in Rust that combines the modal editing approach of Vim with modern features and a cleaner interface. Its primary goals are speed, simplicity, and user-friendliness while maintaining the efficiency of modal editing.

## Features

- **Multiple Editing Modes** - Normal, Insert, Command, and special modes like FileFinder and ReloadConfirm
- **Tab Management** - Easily work with multiple files in separate tabs
- **Smart File Reload** - Intelligent diffing shows exactly what changed on disk
- **Fuzzy File Finding** - Quickly navigate to files with fuzzy matching
- **Rust Integration** - Built-in support for Cargo Check/Clippy for Rust projects
- **Configurable** - Customize key bindings via TOML configuration
- **Visual Feedback** - Clear indicators for modifications, diffs, and diagnostics
- **Modern UI** - Clean interface with informative status line and tab bar

## Key Differences from Vim

- **Simplified Command Structure** - More intuitive commands with less cryptic syntax
- **Enhanced Tab Interface** - Direct access to tabs with F1-F12 keys and clear tab visualization
- **Smart Diff Highlighting** - More accurate diff highlighting when reloading files
- **Built-in Rust Support** - Integrated Cargo commands and diagnostic display
- **Modern Error Handling** - Clear error messages and visual indicators

## Installation

### From source

```bash
git clone https://github.com/yourusername/zim.git
cd zim
cargo build --release
```

The binary will be available at `target/release/zim`.

## Usage

```
zim [FILE]
```

## Key Bindings

### Normal Mode
- `h`, `j`, `k`, `l` - Move left, down, up, right
- `i` - Enter Insert mode
- `:` - Enter Command mode
- `ESC` - Return to Normal mode (from any mode)
- `^` - Move to start of line
- `$` - Move to end of line
- `g` - Move to top of file
- `G` - Move to bottom of file
- `w` - Save file (with confirmation)
- `e` - Reload file with intelligent diff highlighting
- `x` - Save and quit
- `q` - Quit

### Tab Management
- `Ctrl+n` - New tab
- `Ctrl+w` - Close tab
- `Ctrl+left/right` - Previous/Next tab
- `F1-F12` - Direct access to tabs 1-12

### File Operations
- `Ctrl+p` - Find file

### Rust Integration
- `Ctrl+d` - Run cargo check
- `Ctrl+y` - Run cargo clippy

### Help System
- `Ctrl+h` - Show help page with command reference

### Insert Mode
- `ESC` - Return to Normal mode
- Any character - Insert at cursor

### File Finder Mode
- `ESC` - Cancel and return to Normal mode
- `Enter` - Open selected file
- `Up`/`Down` - Navigate through file list
- Type to search with fuzzy matching

## Configuration

Zim is configured through TOML files located at:
- Linux/macOS: 
  - `~/.config/zim/config.toml`: General configuration
  - `~/.config/zim/key_bindings.toml`: Customizable key bindings
- Windows: 
  - `%APPDATA%\zim\config.toml`
  - `%APPDATA%\zim\key_bindings.toml`

The files are created automatically on first run with default settings.

### Customizing Key Bindings

The `key_bindings.toml` file allows you to remap any key in the editor. Here's an example:

```toml
[normal_mode]
quit = { key = "q" }
insert_mode = { key = "i" }
command_mode = { key = ":" }
move_left = { key = "h" }
move_down = { key = "j" }
move_up = { key = "k" }
move_right = { key = "l" }
find_file = { key = "p", modifiers = ["ctrl"] }
reload_file = { key = "e" }
save_file = { key = "w" }
save_and_quit = { key = "x" }

[insert_mode]
normal_mode = { key = "esc" }

[command_mode]
normal_mode = { key = "esc" }

[file_finder_mode]
cancel = { key = "esc" }
select = { key = "enter" }
next = { key = "down" }
previous = { key = "up" }
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT