# Zim

A modern, fast, easily configurable, AI-powered vim from the future.

## Features

- Modal editing (Normal, Insert, Command, FileFinder modes)
- Fast and responsive
- File finder with fuzzy search capabilities
- Configurable through TOML with remappable key bindings
- Modern UI with syntax highlighting (coming soon)
- AI-powered suggestions and autocompletions (coming soon)

## Installation

### From source

```
git clone https://github.com/yourusername/zim.git
cd zim
cargo build --release
```

The binary will be available at `target/release/zim`.

## Usage

```
zim [FILE]
```

### Key Bindings

#### Normal Mode
- `h`, `j`, `k`, `l`: Navigate left, down, up, right
- `i`: Enter Insert mode
- `:`: Enter Command mode
- `q`: Quit
- `Ctrl+p`: Open file finder

#### Insert Mode
- `ESC`: Return to Normal mode
- Any character: Insert at cursor

#### Command Mode
- `ESC`: Return to Normal mode
- (More commands coming soon)

#### File Finder Mode
- `ESC`: Cancel and return to Normal mode
- `Enter`: Open selected file
- `Up`/`Down`: Navigate through file list
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

The `key_bindings.toml` file allows you to remap any key in the editor. Here's an example of how to customize key bindings:

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

You can modify any key binding by changing the `key` value. For special keys like Ctrl, Alt, or Shift, add them to the `modifiers` array.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT