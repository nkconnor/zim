# Zim Editor Information

## Important Information

- NEVER use unsafe code in this codebase
- Maintain type safety and proper ownership/borrowing patterns
- When making changes, use existing patterns and conventions
- Keep the editor's functionality following vim-like keybindings
- Ensure functionality is tested with cargo test
- IMPORTANT: NEVER use `cargo run` directly, this can cause errors

## Common Commands

### Build and Test

```bash
# Build the project
cargo build

# Run tests
cargo test

# Build and run - DO NOT USE
# cargo run
```

## Code Structure

- `src/editor`: Core editor functionality
  - `buffer.rs`: Text buffer management
  - `cursor.rs`: Cursor movement and positioning
  - `mode.rs`: Editor modes (Normal, Insert, Command, etc.)
  - `viewport.rs`: Handles screen viewport and scrolling
  - `file_finder.rs`: File finding functionality
  - `diagnostics.rs`: Diagnostic/error collection and display

- `src/config`: Configuration handling
  - `key_bindings.rs`: Keyboard shortcut mappings
  - `mod.rs`: General configuration

- `src/ui`: User interface rendering
  - Uses tui-rs for terminal UI

- `src/main.rs`: Application entry point and event loop