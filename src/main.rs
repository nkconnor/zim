mod editor;
mod ui;
mod config;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io, time::Duration};
use tui::{
    backend::CrosstermBackend,
    Terminal,
};

use editor::Editor;

/// Zim - the modern, fast, easily configurable, AI powered vim from the future
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Cli {
    /// File to open
    #[clap(name = "FILE")]
    file: Option<String>,
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut editor: Editor,
) -> Result<()> {
    loop {
        // Draw UI and collect any viewport updates
        let mut viewport_update = None;
        terminal.draw(|f| {
            viewport_update = ui::render(f, &mut editor);
        })?;
        
        // Apply viewport updates if needed (safely updates viewport dimensions)
        if let Some(update) = viewport_update {
            if let Some(tab) = editor.tabs.get_mut(editor.current_tab) {
                tab.viewport.width = update.width;
                tab.viewport.height = update.height;
            }
        }

        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle key event in the editor
                if !editor.handle_key(key)? {
                    // Editor returned false, which means we should quit
                    return Ok(());
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Load config
    let config = config::Config::load()?;
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create editor with config
    let mut editor = Editor::new_with_config(config);
    
    // Load file if provided
    if let Some(file_path) = &cli.file {
        editor.load_file(file_path)?;
    }

    let res = run_app(&mut terminal, editor);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}