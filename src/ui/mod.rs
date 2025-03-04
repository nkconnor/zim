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

    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(size);

    match editor.mode {
        Mode::FileFinder => {
            render_file_finder(f, editor, chunks[0]);
        },
        _ => {
            render_editor_area(f, editor, chunks[0]);
        }
    }
    
    render_status_line(f, editor, chunks[1]);
}

fn render_editor_area<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    let editor_block = Block::default()
        .title(" Zim Editor ")
        .borders(Borders::ALL);
    
    // Get the inner area dimensions (accounting for borders)
    let inner_area = editor_block.inner(area);
    
    // Update only the viewport dimensions in the editor (not top_line/left_column)
    // This allows for proper scrolling calculations while avoiding jiggling
    let mut viewport = editor.viewport.clone();
    
    // Reserve space for line numbers (at least 4 chars for thousands of lines)
    let line_number_width = 5; // 4 digits + 1 space
    let content_width = inner_area.width.saturating_sub(line_number_width);
    
    viewport.update_dimensions(content_width as usize, inner_area.height as usize);
    
    // Sync dimensions with editor's viewport, but not the scroll position
    if let Some(editor_mut) = unsafe { (editor as *const Editor as *mut Editor).as_mut() } {
        editor_mut.viewport.width = viewport.width;
        editor_mut.viewport.height = viewport.height;
    }
    
    // Calculate visible range
    let (start_line, end_line) = viewport.get_visible_range(editor.buffer.line_count());
    
    // We'll avoid changing left_column during rendering to prevent jiggling
    let left_column = editor.viewport.left_column;
    
    // Format to get max line number width
    let total_lines = editor.buffer.line_count();
    let line_num_width = total_lines.to_string().len();
    
    // Convert only visible buffer lines to Lines for rendering with line numbers
    let lines: Vec<Line> = editor.buffer.lines[start_line..end_line].iter()
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
            if let Some(line_diagnostics) = editor.diagnostics.get_diagnostics_for_line(current_line) {
                // If there are diagnostics, create styled spans based on the diagnostics
                if !line_diagnostics.is_empty() && !content.is_empty() {
                    let mut remaining = content.clone();
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
    let cursor_x = editor.cursor.x.saturating_sub(left_column);
    let cursor_y = editor.cursor.y.saturating_sub(viewport.top_line);
    
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

fn render_status_line<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    let mode_text = match editor.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Command => "COMMAND",
        Mode::FileFinder => "FILE FINDER",
    };
    
    let status = match editor.mode {
        Mode::FileFinder => format!("{} | Press Enter to select, Esc to cancel", mode_text),
        _ => {
            let total_lines = editor.buffer.line_count();
            let viewport = &editor.viewport;
            let top_percent = if total_lines > 0 {
                (viewport.top_line * 100) / total_lines
            } else {
                0
            };
            
            format!("{} | Ln: {}/{} ({}%), Col: {}", 
                mode_text, 
                editor.cursor.y + 1, 
                total_lines,
                top_percent,
                editor.cursor.x + 1
            )
        },
    };
    
    let status_bar = Paragraph::new(status)
        .style(Style::default().bg(Color::Blue).fg(Color::White));
    
    f.render_widget(status_bar, area);
}