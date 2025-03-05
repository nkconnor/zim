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
git clone https://github.com/nkconnor/zim.git
cd zim
cargo build --release
```

The binary will be available at `target/release/zim`.

Alternatively, just run `cargo run --release`.

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

### Editing
- `d` - Delete current line
- `x` - Delete character and enter insert mode
- `o` - Open new line below cursor and enter insert mode
- `O` - Open new line above cursor and enter insert mode
- `u` - Undo last action
- `Ctrl+r` - Redo previously undone action

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
# Example showing basic customization
[normal_mode]
save_file = { key = "w" }
reload_file = { key = "e" }
quit = { key = "q" }

[insert_mode]
normal_mode = { key = "esc" }
```

### Available Commands for Keybinding Customization

#### Normal Mode Commands
```toml
[normal_mode]
# File operations
quit = { key = "q" }                         # Quit editor
save_file = { key = "w" }                    # Save current file
reload_file = { key = "e" }                  # Reload file from disk
save_and_quit = { key = "X" }                # Save and quit

# Mode switching
insert_mode = { key = "i" }                  # Enter insert mode
show_help = { key = "h", modifiers = ["ctrl"] } # Show help

# Navigation
move_left = { key = "h" }                    # Move cursor left
move_down = { key = "j" }                    # Move cursor down
move_up = { key = "k" }                      # Move cursor up
move_right = { key = "l" }                   # Move cursor right
move_to_line_start = { key = "^" }           # Move to start of line
move_to_line_end = { key = "$" }             # Move to end of line
move_to_file_start = { key = "g" }           # Move to top of file
move_to_file_end = { key = "G" }             # Move to bottom of file
page_up = { key = "b", modifiers = ["ctrl"] }    # Page up
page_down = { key = "f", modifiers = ["ctrl"] }  # Page down

# Editing operations
delete_line = { key = "d" }                  # Delete current line
delete_char = { key = "x" }                  # Delete character and enter insert mode
open_line_below = { key = "o" }              # Open new line below cursor and enter insert mode
open_line_above = { key = "O" }              # Open new line above cursor and enter insert mode
undo = { key = "u" }                         # Undo last action
redo = { key = "r", modifiers = ["ctrl"] }   # Redo previously undone action

# Tab management
new_tab = { key = "n", modifiers = ["ctrl"] }        # Create new tab
close_tab = { key = "w", modifiers = ["ctrl"] }      # Close current tab
next_tab = { key = "right", modifiers = ["ctrl"] }   # Go to next tab
prev_tab = { key = "left", modifiers = ["ctrl"] }    # Go to previous tab
goto_tab_1 = { key = "f1" }                  # Go to tab 1
goto_tab_2 = { key = "f2" }                  # Go to tab 2
# ... through goto_tab_12 = { key = "f12" }

# Features
find_file = { key = "o", modifiers = ["ctrl"] }      # Open file finder
token_search = { key = "t", modifiers = ["ctrl"] }   # Search for tokens
run_cargo_check = { key = "d", modifiers = ["ctrl"] } # Run cargo check
run_cargo_clippy = { key = "y", modifiers = ["ctrl"] } # Run cargo clippy
snake_game = { key = "s" }                   # Easter egg: launch snake game
```

#### Insert Mode Commands
```toml
[insert_mode]
normal_mode = { key = "esc" }                # Return to normal mode
```

#### Command Mode Commands
```toml
[command_mode]
normal_mode = { key = "esc" }                # Return to normal mode
```

#### File Finder Mode Commands
```toml
[file_finder_mode]
cancel = { key = "esc" }                     # Cancel file finder
select = { key = "enter" }                   # Select file
next = { key = "down" }                      # Next file
previous = { key = "up" }                    # Previous file
```

#### Token Search Mode Commands
```toml
[token_search_mode]
cancel = { key = "esc" }                     # Cancel token search
select = { key = "enter" }                   # Select result
next = { key = "down" }                      # Next result
previous = { key = "up" }                    # Previous result
```

#### Help Mode Commands
```toml
[help_mode]
normal_mode = { key = "esc" }                # Return to normal mode
```

You can specify key modifiers using the `modifiers` array:
```toml
# Example with modifiers
save_file = { key = "s", modifiers = ["ctrl"] }      # Ctrl+S to save
quit = { key = "q", modifiers = ["ctrl", "shift"] }  # Ctrl+Shift+Q to quit
```

Valid modifiers are: `"ctrl"`, `"alt"`, and `"shift"`.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License