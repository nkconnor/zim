use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::editor::{Editor, Mode};

pub fn render<B: Backend>(f: &mut Frame<B>, editor: &Editor) {
    let size = f.size();

    // Create the layout with tab bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Tab bar - increased height for visibility
            Constraint::Min(1),    // Editor area
            Constraint::Length(1)  // Status line
        ].as_ref())
        .split(size);

    // Render the tab bar
    render_tab_bar(f, editor, chunks[0]);
    
    // Render main content
    match editor.mode {
        Mode::FileFinder => {
            render_file_finder(f, editor, chunks[1]);
        },
        Mode::Help => {
            render_help_page(f, editor, chunks[1]);
        },
        _ => {
            render_editor_area(f, editor, chunks[1]);
        }
    }
    
    // Render status line
    render_status_line(f, editor, chunks[2]);
}

/// Render the tab bar
fn render_tab_bar<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    // Make the tab bar more visible with a bright title and yellow border
    let tab_bar_block = Block::default()
        .title(" TABS ")
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    
    let inner_area = tab_bar_block.inner(area);
    f.render_widget(tab_bar_block, area);
    
    // Create tab items
    let mut tab_spans = Vec::new();
    
    // Debug - always show at least one tab
    if editor.tabs.is_empty() {
        tab_spans.push(tui::text::Span::styled(
            " F1 untitled ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }
    
    for (idx, tab) in editor.tabs.iter().enumerate() {
        // Get filename for tab or show untitled
        let filename = match &tab.buffer.file_path {
            Some(path) => {
                let path_str = path.as_str();
                // Check if it's a path or just a name
                if path_str.contains('/') {
                    if let Some(filename) = std::path::Path::new(path_str).file_name() {
                        filename.to_string_lossy().to_string()
                    } else {
                        path_str.to_string()
                    }
                } else {
                    // It's just a name, use it directly
                    path_str.to_string()
                }
            },
            None => format!("untitled-{}", idx + 1),
        };
        
        // Style for current tab vs other tabs
        let style = if idx == editor.current_tab {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        
        // Add F-key number for tab (show F1-F12 for tabs 1-12)
        let f_key_display = if idx < 12 {
            format!("F{} ", idx + 1)
        } else {
            "".to_string()
        };
        
        // Add tab item
        tab_spans.push(tui::text::Span::styled(
            format!(" {}{} ", f_key_display, filename),
            style,
        ));
        
        // Add separator
        tab_spans.push(tui::text::Span::raw(" | "));
    }
    
    // Remove last separator if any tabs
    if !tab_spans.is_empty() {
        tab_spans.pop();
    }
    
    // Add tab controls hint
    tab_spans.push(tui::text::Span::styled(
        " (Ctrl+n: New, Ctrl+w: Close, F1-F12: Direct access, Ctrl+left/right: Prev/Next) ",
        Style::default().fg(Color::DarkGray),
    ));
    
    // Create tab line - add debug count so we always see something
    let debug_count = format!(" [DEBUG: {} tabs] ", editor.tabs.len());
    tab_spans.push(tui::text::Span::styled(debug_count, Style::default().fg(Color::Red)));
    
    let tabs_line = Line::from(tab_spans);
    
    // Create a distinct paragraph with visible background color
    let tabs_paragraph = Paragraph::new(vec![tabs_line])
        .alignment(tui::layout::Alignment::Left)
        .style(Style::default().bg(Color::Black));
    
    // Make sure we have enough space to render
    if inner_area.width > 1 && inner_area.height > 0 {
        f.render_widget(tabs_paragraph, inner_area);
    } else {
        // Area is too small - this helps diagnose layout issues
        let debug_para = Paragraph::new("AREA TOO SMALL")
            .style(Style::default().fg(Color::Red));
        f.render_widget(debug_para, area);
    }
}

fn render_editor_area<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    // Get the current tab
    let tab = editor.current_tab();
    
    let editor_block = Block::default()
        .title(" Zim Editor ")
        .borders(Borders::ALL);
    
    // Get the inner area dimensions (accounting for borders)
    let inner_area = editor_block.inner(area);
    
    // Update only the viewport dimensions in the editor (not top_line/left_column)
    // This allows for proper scrolling calculations while avoiding jiggling
    let mut viewport = tab.viewport.clone();
    
    // Reserve space for line numbers (at least 4 chars for thousands of lines)
    let line_number_width = 5; // 4 digits + 1 space
    let content_width = inner_area.width.saturating_sub(line_number_width);
    
    viewport.update_dimensions(content_width as usize, inner_area.height as usize);
    
    // Sync dimensions with editor's viewport, but not the scroll position
    if let Some(editor_mut) = unsafe { (editor as *const Editor as *mut Editor).as_mut() } {
        if let Some(tab_mut) = editor_mut.tabs.get_mut(editor_mut.current_tab) {
            tab_mut.viewport.width = viewport.width;
            tab_mut.viewport.height = viewport.height;
        }
    }
    
    // Calculate visible range
    let (start_line, end_line) = viewport.get_visible_range(tab.buffer.line_count());
    
    // We'll avoid changing left_column during rendering to prevent jiggling
    let left_column = tab.viewport.left_column;
    
    // Format to get max line number width
    let total_lines = tab.buffer.line_count();
    let line_num_width = total_lines.to_string().len();
    
    // Convert only visible buffer lines to Lines for rendering with line numbers
    let lines: Vec<Line> = tab.buffer.lines[start_line..end_line].iter()
        .enumerate()
        .map(|(idx, line)| {
            let line_number = start_line + idx + 1; // 1-indexed line numbers
            let number_str = format!("{:>width$} ", line_number, width=line_num_width);
            
            // Create line with number followed by content
            let mut spans = vec![
                tui::text::Span::styled(
                    number_str, 
                    Style::default().fg(Color::DarkGray)
                )
            ];
            
            // Add the actual line content with diagnostic highlighting if needed
            let content = if left_column < line.len() {
                line[left_column.min(line.len())..].to_string()
            } else {
                "".to_string()
            };
            
            // Check if there are diagnostics for this line
            let current_line = start_line + idx;
            if let Some(line_diagnostics) = tab.diagnostics.get_diagnostics_for_line(current_line) {
                // If there are diagnostics, create styled spans based on the diagnostics
                if !line_diagnostics.is_empty() && !content.is_empty() {
                    let mut pos = 0;
                    let mut content_spans = Vec::new();
                    
                    // Sort diagnostics by start_column
                    let mut sorted_diags = line_diagnostics.clone();
                    sorted_diags.sort_by_key(|d| d.span.start_column);
                    
                    for diag in sorted_diags {
                        let start = diag.span.start_column.saturating_sub(left_column);
                        let end = diag.span.end_column.saturating_sub(left_column);
                        
                        // Skip if the diagnostic is outside the visible range
                        if start >= content.len() || end <= 0 {
                            continue;
                        }
                        
                        // Add text before the diagnostic
                        if start > pos {
                            let before_text = &content[pos..start];
                            content_spans.push(tui::text::Span::raw(before_text.to_string()));
                        }
                        
                        // Add the diagnostic with styling
                        let diagnostic_style = match diag.severity {
                            crate::editor::DiagnosticSeverity::Error => {
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                            },
                            crate::editor::DiagnosticSeverity::Warning => {
                                Style::default().fg(Color::Yellow)
                            },
                            crate::editor::DiagnosticSeverity::Information => {
                                Style::default().fg(Color::Blue).add_modifier(Modifier::ITALIC)
                            },
                            crate::editor::DiagnosticSeverity::Hint => {
                                Style::default().fg(Color::Green).add_modifier(Modifier::ITALIC)
                            },
                        };
                        
                        let end_idx = std::cmp::min(end, content.len());
                        if start < end_idx {
                            let diagnostic_text = &content[start..end_idx];
                            content_spans.push(tui::text::Span::styled(
                                diagnostic_text.to_string(),
                                diagnostic_style,
                            ));
                        }
                        
                        pos = end_idx;
                    }
                    
                    // Add remaining text after the last diagnostic
                    if pos < content.len() {
                        let after_text = &content[pos..];
                        content_spans.push(tui::text::Span::raw(after_text.to_string()));
                    }
                    
                    // Add all spans to the line
                    spans.extend(content_spans);
                } else {
                    spans.push(tui::text::Span::raw(content));
                }
            } else {
                // No diagnostics, just add the raw content
                spans.push(tui::text::Span::raw(content));
            }
            
            Line::from(spans)
        })
        .collect();
    
    let paragraph = Paragraph::new(lines)
        .block(editor_block)
        .style(Style::default())
        .wrap(Wrap { trim: false });
    
    f.render_widget(paragraph, area);

    // Set cursor position relative to viewport
    // Use the same left_column we used for rendering to ensure consistency
    let cursor_x = tab.cursor.x.saturating_sub(left_column);
    let cursor_y = tab.cursor.y.saturating_sub(viewport.top_line);
    
    // Adjust cursor position for line numbers
    // Add the number width to the cursor x position
    let line_number_offset = line_num_width + 1; // width + space
    
    f.set_cursor(
        area.x + cursor_x as u16 + line_number_offset as u16 + 1, // +1 for the border
        area.y + cursor_y as u16 + 1, // +1 for the border
    );
}

fn render_file_finder<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    // Create a block for the file finder
    let file_finder_block = Block::default()
        .title(" Find File ")
        .borders(Borders::ALL);

    let inner_area = file_finder_block.inner(area);
    f.render_widget(file_finder_block, area);

    // Create search input area at the top
    let search_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
        ].as_ref())
        .split(inner_area);

    // Render search query
    let search_block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL);
    
    let search_text = Paragraph::new(editor.file_finder.query())
        .block(search_block)
        .style(Style::default());
    
    f.render_widget(search_text, search_layout[0]);

    // Render file list
    let list_block = Block::default()
        .title(" Files ")
        .borders(Borders::ALL);

    let matches = editor.file_finder.matches();
    let selected_index = editor.file_finder.selected_index();
    
    let items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .map(|(i, (path, _score))| {
            let style = if i == selected_index {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            ListItem::new(path.clone()).style(style)
        })
        .collect();

    let file_list = List::new(items)
        .block(list_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    
    f.render_widget(file_list, search_layout[1]);

    // Set cursor at the end of the search query
    f.set_cursor(
        search_layout[0].x + editor.file_finder.query().len() as u16 + 1,
        search_layout[0].y + 1,
    );
}

fn render_help_page<B: Backend>(f: &mut Frame<B>, _editor: &Editor, area: Rect) {
    let help_block = Block::default()
        .title(" Help - Press ESC to exit ")
        .borders(Borders::ALL);
    
    let inner_area = help_block.inner(area);
    f.render_widget(help_block, area);
    
    // Create sections of help content
    let mut text = Vec::new();
    
    // Title section
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "ZIM EDITOR KEYBOARD SHORTCUTS",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        )
    ]));
    text.push(Line::from(""));
    
    // Normal Mode section
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "NORMAL MODE",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        )
    ]));
    text.push(Line::from(""));
    
    // Basic navigation
    text.push(Line::from(vec![
        tui::text::Span::styled("Basic Navigation:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("h, j, k, l - Move left, down, up, right"));
    text.push(Line::from("^ - Move to start of line"));
    text.push(Line::from("$ - Move to end of line"));
    text.push(Line::from("g - Move to top of file"));
    text.push(Line::from("G - Move to bottom of file"));
    text.push(Line::from("Ctrl+b - Page up"));
    text.push(Line::from("Ctrl+f - Page down"));
    text.push(Line::from(""));
    
    // Modes
    text.push(Line::from(vec![
        tui::text::Span::styled("Mode Switching:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("i - Enter Insert mode"));
    text.push(Line::from(": - Enter Command mode"));
    text.push(Line::from("ESC - Return to Normal mode (from any mode)"));
    text.push(Line::from(""));
    
    // Tabs
    text.push(Line::from(vec![
        tui::text::Span::styled("Tab Management:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+n - New tab"));
    text.push(Line::from("Ctrl+w - Close tab"));
    text.push(Line::from("Ctrl+right - Next tab"));
    text.push(Line::from("Ctrl+left - Previous tab"));
    text.push(Line::from("F1-F12 - Switch directly to tabs 1-12"));
    text.push(Line::from(""));
    
    // File operations
    text.push(Line::from(vec![
        tui::text::Span::styled("File Operations:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+p - Find file"));
    text.push(Line::from("q - Quit (in normal mode)"));
    text.push(Line::from(""));
    
    // Cargo integration
    text.push(Line::from(vec![
        tui::text::Span::styled("Cargo Integration:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+d - Run cargo check"));
    text.push(Line::from("Ctrl+y - Run cargo clippy"));
    text.push(Line::from(""));
    
    // Help
    text.push(Line::from(vec![
        tui::text::Span::styled("Help:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+h - Show this help page"));
    text.push(Line::from("ESC or q - Exit help and return to normal mode"));
    
    // Render the help text
    let help_text = Paragraph::new(text)
        .alignment(tui::layout::Alignment::Left)
        .scroll((0, 0));
    
    f.render_widget(help_text, inner_area);
}

fn render_status_line<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    let mode_text = match editor.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Command => "COMMAND",
        Mode::FileFinder => "FILE FINDER",
        Mode::Help => "HELP",
    };
    
    let status = match editor.mode {
        Mode::FileFinder => format!("{} | Press Enter to select, Esc to cancel", mode_text),
        _ => {
            // Get current tab info
            let tab = editor.current_tab();
            let total_lines = tab.buffer.line_count();
            let viewport = &tab.viewport;
            let top_percent = if total_lines > 0 {
                (viewport.top_line * 100) / total_lines
            } else {
                0
            };
            
            // Get filename if available
            let file_info = match &tab.buffer.file_path {
                Some(path) => {
                    if let Some(filename) = std::path::Path::new(path).file_name() {
                        filename.to_string_lossy().to_string()
                    } else {
                        "untitled".to_string()
                    }
                },
                None => "untitled".to_string(),
            };
            
            // Get diagnostic count for the current file
            let error_count = tab.diagnostics.get_all_diagnostics().iter()
                .filter(|d| d.severity == crate::editor::DiagnosticSeverity::Error)
                .count();
            
            let warning_count = tab.diagnostics.get_all_diagnostics().iter()
                .filter(|d| d.severity == crate::editor::DiagnosticSeverity::Warning)
                .count();
            
            // Create diagnostic indicators
            let diagnostic_info = if error_count > 0 || warning_count > 0 {
                format!(" | ❌ {} ⚠️ {}", error_count, warning_count)
            } else {
                "".to_string()
            };
            
            format!("{} | {} | Tab {}/{} | Ln: {}/{} ({}%), Col: {}{}", 
                mode_text,
                file_info, 
                editor.current_tab + 1,
                editor.tabs.len(),
                tab.cursor.y + 1, 
                total_lines,
                top_percent,
                tab.cursor.x + 1,
                diagnostic_info
            )
        },
    };
    
    let status_bar = Paragraph::new(status)
        .style(Style::default().bg(Color::Blue).fg(Color::White));
    
    f.render_widget(status_bar, area);
}