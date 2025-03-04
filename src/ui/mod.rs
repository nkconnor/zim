use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::editor::{Editor, Mode, HighlightedLine, Tab};
use std::collections::HashMap;
use syntect::highlighting::Style as SyntectStyle;

/// Holds information about viewport dimensions that need to be updated
pub struct ViewportUpdate {
    pub width: usize,
    pub height: usize,
}

pub fn render<B: Backend>(f: &mut Frame<B>, editor: &mut Editor) -> Option<ViewportUpdate> {
    let size = f.size();
    let mut viewport_update = None;

    // Create the layout with tab bar (increased height)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Tab bar - increased for visibility
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
        Mode::WriteConfirm => {
            // In WriteConfirm mode, we still show the editor but highlight modified lines
            viewport_update = render_editor_area_with_highlights(f, editor, chunks[1], false);
        },
        Mode::ReloadConfirm => {
            // In ReloadConfirm mode, we show the editor with diff lines highlighted
            viewport_update = render_editor_area_with_diff_highlights(f, editor, chunks[1]);
        },
        Mode::FilenamePrompt => {
            render_filename_prompt(f, editor, chunks[1]);
        },
        Mode::Visual | Mode::VisualLine => {
            // In Visual modes, highlight the selection
            viewport_update = render_editor_area_with_selection(f, editor, chunks[1]);
        },
        _ => {
            viewport_update = render_editor_area(f, editor, chunks[1]);
        }
    }
    
    // Render status line
    render_status_line(f, editor, chunks[2]);
    
    viewport_update
}

/// Render the tab bar
fn render_tab_bar<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    // Create the tab bar block with prominent coloring
    let tab_bar_block = Block::default()
        .title(" TABS ")
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    
    let inner_area = tab_bar_block.inner(area);
    f.render_widget(tab_bar_block, area);
    
    // Create tab items
    let mut tab_spans = Vec::new();
    
    // Always show debug info to confirm tabs are rendering
    tab_spans.push(tui::text::Span::styled(
        format!(" [{}] ", editor.tabs.len()),
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    ));
    
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
    
    // Create tab line
    let tabs_line = Line::from(tab_spans);
    
    // Create the paragraph for tabs display
    let tabs_paragraph = Paragraph::new(vec![tabs_line])
        .alignment(tui::layout::Alignment::Left)
        .style(Style::default().bg(Color::Black));
    
    // Render the tabs
    f.render_widget(tabs_paragraph, inner_area);
}

// Common rendering function that can optionally highlight modified or diff lines
fn render_editor_area_inner<B: Backend>(
    f: &mut Frame<B>, 
    editor: &mut Editor, 
    area: Rect, 
    highlight_modified: bool,
    is_diff_mode: bool
) -> Option<ViewportUpdate> {
    // Create a cache for highlighted lines
    let mut highlight_cache = HashMap::new();
    std::mem::swap(&mut highlight_cache, &mut editor.highlighted_lines_cache);
    
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
            let current_line_idx = start_line + idx;
            let is_modified = tab.buffer.is_line_modified(current_line_idx);
            let is_diff = editor.diff_lines.contains(&current_line_idx);
            
            // Style the line number based on modification/diff status if highlighting is enabled
            let number_style = if highlight_modified {
                if is_diff_mode && is_diff {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else if !is_diff_mode && is_modified {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                }
            } else {
                Style::default().fg(Color::DarkGray)
            };
            
            let number_str = format!("{:>width$} ", line_number, width=line_num_width);
            
            // Create line with number followed by content
            let mut spans = vec![
                tui::text::Span::styled(number_str, number_style)
            ];
            
            // Add the actual line content with diagnostic or syntax highlighting as needed
            let content = if left_column < line.len() {
                line[left_column.min(line.len())..].to_string()
            } else {
                "".to_string()
            };
            
            // Choose whether to add diagnostic, modification, or diff highlighting
            let current_line = start_line + idx;
            
            // Start with basic styling decisions
            if highlight_modified {
                if is_diff_mode && is_diff {
                    // In ReloadConfirm mode, highlight the entire diff line in yellow
                    spans.push(tui::text::Span::styled(
                        content,
                        Style::default().fg(Color::Yellow)
                    ));
                } else if !is_diff_mode && is_modified {
                    // In WriteConfirm mode, highlight the entire modified line in green
                    spans.push(tui::text::Span::styled(
                        content,
                        Style::default().fg(Color::Green)
                    ));
                } else if let Some(line_diagnostics) = tab.diagnostics.get_diagnostics_for_line(current_line) {
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
                        // Apply syntax highlighting if available
                        if let Some(syntax_ref) = &tab.buffer.syntax {
                            // Get line for highlighting
                            let line_for_highlight = if current_line < tab.buffer.lines.len() {
                                &tab.buffer.lines[current_line]
                            } else {
                                ""
                            };
                            
                            // Use the cache to avoid recomputing syntax highlights
                            let tab_idx = editor.current_tab;
                            let cache_key = (tab_idx, current_line);
                            
                            // Get or compute the highlighted line
                            let highlighted = if let Some(cached) = highlight_cache.get(&cache_key) {
                                cached.clone()
                            } else {
                                // Highlight the line
                                let highlighted = editor.syntax_highlighter.highlight_text(
                                    &format!("{}\n", line_for_highlight), 
                                    syntax_ref.clone()
                                );
                                
                                // Store for later caching
                                let result = highlighted.clone();
                                highlight_cache.insert(cache_key, result);
                                highlighted
                            };
                            
                            if !highlighted.is_empty() {
                                let line_spans = highlighted[0].ranges.iter()
                                    .filter_map(|(style, text)| {
                                        // Skip empty text
                                        if text.is_empty() { 
                                            return None; 
                                        }
                                        
                                        // Convert syntect style to tui style
                                        let tui_style = convert_syntect_style(style);
                                        
                                        // Create the span
                                        Some(Span::styled(text.clone(), tui_style))
                                    })
                                    .collect::<Vec<_>>();
                                
                                spans.extend(line_spans);
                            } else {
                                spans.push(tui::text::Span::raw(content));
                            }
                        } else {
                            spans.push(tui::text::Span::raw(content));
                        }
                    }
                } else {
                    // Apply syntax highlighting if available
                    if let Some(syntax_ref) = &tab.buffer.syntax {
                        // Get line for highlighting
                        let line_for_highlight = if current_line < tab.buffer.lines.len() {
                            &tab.buffer.lines[current_line]
                        } else {
                            ""
                        };
                        
                        // Use the cache to avoid recomputing syntax highlights
                        let tab_idx = editor.current_tab;
                        let cache_key = (tab_idx, current_line);
                        
                        // Get or compute the highlighted line
                        let highlighted = if let Some(cached) = highlight_cache.get(&cache_key) {
                            cached.clone()
                        } else {
                            // Highlight the line
                            let highlighted = editor.syntax_highlighter.highlight_text(
                                &format!("{}\n", line_for_highlight), 
                                syntax_ref.clone()
                            );
                            
                            // Store for later caching
                            let result = highlighted.clone();
                            highlight_cache.insert(cache_key, result);
                            highlighted
                        };
                        
                        // Use the helper function to create highlighted spans
                        let line_spans = create_highlighted_spans(&highlighted);
                        if !line_spans.is_empty() {
                            spans.extend(line_spans);
                        } else {
                            spans.push(tui::text::Span::raw(content));
                        }
                    } else {
                        spans.push(tui::text::Span::raw(content));
                    }
                }
            } else {
                // Apply syntax highlighting if available
                if let Some(syntax_ref) = &tab.buffer.syntax {
                    // Get line for highlighting
                    let line_for_highlight = if current_line < tab.buffer.lines.len() {
                        &tab.buffer.lines[current_line]
                    } else {
                        ""
                    };
                    
                    // Use the cache to avoid recomputing syntax highlights
                    let tab_idx = editor.current_tab;
                    let cache_key = (tab_idx, current_line);
                    
                    // Get or compute the highlighted line
                    let highlighted = if let Some(cached) = highlight_cache.get(&cache_key) {
                        cached.clone()
                    } else {
                        // Highlight the line
                        let highlighted = editor.syntax_highlighter.highlight_text(
                            &format!("{}\n", line_for_highlight), 
                            syntax_ref.clone()
                        );
                        
                        // Store for later caching
                        let result = highlighted.clone();
                        highlight_cache.insert(cache_key, result);
                        highlighted
                    };
                    
                    // Use the helper function to create highlighted spans
                    let line_spans = create_highlighted_spans(&highlighted);
                    if !line_spans.is_empty() {
                        spans.extend(line_spans);
                    } else {
                        spans.push(tui::text::Span::raw(content));
                    }
                } else {
                    spans.push(tui::text::Span::raw(content));
                }
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
    
    // Restore the cache
    std::mem::swap(&mut highlight_cache, &mut editor.highlighted_lines_cache);
    
    // Return viewport dimensions for safe update
    Some(ViewportUpdate {
        width: viewport.width,
        height: viewport.height,
    })
}

fn render_editor_area<B: Backend>(f: &mut Frame<B>, editor: &mut Editor, area: Rect) -> Option<ViewportUpdate> {
    render_editor_area_inner(f, editor, area, false, false)
}

fn render_editor_area_with_highlights<B: Backend>(f: &mut Frame<B>, editor: &mut Editor, area: Rect, is_reload_mode: bool) -> Option<ViewportUpdate> {
    render_editor_area_inner(f, editor, area, true, is_reload_mode)
}

fn render_editor_area_with_diff_highlights<B: Backend>(f: &mut Frame<B>, editor: &mut Editor, area: Rect) -> Option<ViewportUpdate> {
    render_editor_area_inner(f, editor, area, true, true)
}

/// Render editor area with highlighted selection
fn render_editor_area_with_selection<B: Backend>(f: &mut Frame<B>, editor: &mut Editor, area: Rect) -> Option<ViewportUpdate> {
    // Create a block for the editor
    let editor_block = Block::default()
        .title(" Zim Editor ")
        .borders(Borders::ALL);
    
    // Get the inner area dimensions (accounting for borders)
    let inner_area = editor_block.inner(area);
    
    // Get the current tab
    let tab = editor.current_tab();
    
    // Update viewport dimensions for proper rendering
    let mut viewport = tab.viewport.clone();
    
    // Reserve space for line numbers (at least 4 chars for thousands of lines)
    let line_number_width = 5; // 4 digits + 1 space
    let content_width = inner_area.width.saturating_sub(line_number_width);
    
    viewport.update_dimensions(content_width as usize, inner_area.height as usize);
    
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
            let current_line_idx = start_line + idx;
            
            // Style the line number
            let number_style = Style::default().fg(Color::DarkGray);
            let number_str = format!("{:>width$} ", line_number, width=line_num_width);
            
            // Create line with number followed by content
            let mut spans = vec![
                tui::text::Span::styled(number_str, number_style)
            ];
            
            // If there are no syntax highlighting or diagnostics, and the buffer uses a selection,
            // we need to render the line with selected portions highlighted
            let content = if left_column < line.len() {
                line[left_column.min(line.len())..].to_string()
            } else {
                "".to_string()
            };
            
            // Check diagnostics first
            let current_line = start_line + idx;
            if let Some(line_diagnostics) = tab.diagnostics.get_diagnostics_for_line(current_line) {
                if !line_diagnostics.is_empty() && !content.is_empty() {
                    // Handle diagnostic highlighting (same as in render_editor_area_inner)
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
                        
                        // Add text before the diagnostic, checking if it's selected
                        if start > pos {
                            let before_text = &content[pos..start];
                            let mut char_pos = pos;
                            let mut selected_spans = Vec::new();
                            let mut current_selected = false;
                            let mut segment_start = 0;
                            
                            // Check each character if it's selected
                            for (i, _) in before_text.char_indices() {
                                let col = left_column + char_pos + i;
                                let is_selected = tab.buffer.is_position_selected(
                                    current_line, 
                                    col, 
                                    &tab.cursor, 
                                    tab.buffer.selection_start.is_some() && editor.mode == crate::editor::Mode::VisualLine
                                );
                                
                                if is_selected != current_selected {
                                    // Transition between selected/unselected
                                    if segment_start < i {
                                        let segment = &before_text[segment_start..i];
                                        if current_selected {
                                            selected_spans.push(tui::text::Span::styled(
                                                segment.to_string(),
                                                Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD)
                                            ));
                                        } else {
                                            selected_spans.push(tui::text::Span::raw(segment.to_string()));
                                        }
                                    }
                                    segment_start = i;
                                    current_selected = is_selected;
                                }
                            }
                            
                            // Add the final segment
                            if segment_start < before_text.len() {
                                let segment = &before_text[segment_start..];
                                if current_selected {
                                    selected_spans.push(tui::text::Span::styled(
                                        segment.to_string(),
                                        Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD)
                                    ));
                                } else {
                                    selected_spans.push(tui::text::Span::raw(segment.to_string()));
                                }
                            }
                            
                            content_spans.extend(selected_spans);
                            char_pos += before_text.len();
                        }
                        
                        // Add the diagnostic with styling, checking if it's selected
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
                            
                            // Check if diagnostic text is in selection
                            let mut char_pos = start;
                            let mut selected_spans = Vec::new();
                            let mut current_selected = false;
                            let mut segment_start = 0;
                            
                            // Check each character if it's selected
                            for (i, _) in diagnostic_text.char_indices() {
                                let col = left_column + char_pos + i;
                                let is_selected = tab.buffer.is_position_selected(
                                    current_line, 
                                    col, 
                                    &tab.cursor, 
                                    tab.buffer.selection_start.is_some() && editor.mode == crate::editor::Mode::VisualLine
                                );
                                
                                if is_selected != current_selected {
                                    // Transition between selected/unselected
                                    if segment_start < i {
                                        let segment = &diagnostic_text[segment_start..i];
                                        let style = if current_selected {
                                            diagnostic_style.patch(Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD))
                                        } else {
                                            diagnostic_style
                                        };
                                        selected_spans.push(tui::text::Span::styled(segment.to_string(), style));
                                    }
                                    segment_start = i;
                                    current_selected = is_selected;
                                }
                            }
                            
                            // Add the final segment
                            if segment_start < diagnostic_text.len() {
                                let segment = &diagnostic_text[segment_start..];
                                let style = if current_selected {
                                    diagnostic_style.patch(Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD))
                                } else {
                                    diagnostic_style
                                };
                                selected_spans.push(tui::text::Span::styled(segment.to_string(), style));
                            }
                            
                            content_spans.extend(selected_spans);
                            char_pos += diagnostic_text.len();
                        }
                        
                        pos = end_idx;
                    }
                    
                    // Add remaining text after the last diagnostic
                    if pos < content.len() {
                        let after_text = &content[pos..];
                        
                        // Check if remaining text is in selection
                        let mut char_pos = pos;
                        let mut selected_spans = Vec::new();
                        let mut current_selected = false;
                        let mut segment_start = 0;
                        
                        // Check each character if it's selected
                        for (i, _) in after_text.char_indices() {
                            let col = left_column + char_pos + i;
                            let is_selected = tab.buffer.is_position_selected(
                                current_line, 
                                col, 
                                &tab.cursor, 
                                tab.buffer.selection_start.is_some() && editor.mode == crate::editor::Mode::VisualLine
                            );
                            
                            if is_selected != current_selected {
                                // Transition between selected/unselected
                                if segment_start < i {
                                    let segment = &after_text[segment_start..i];
                                    if current_selected {
                                        selected_spans.push(tui::text::Span::styled(
                                            segment.to_string(),
                                            Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD)
                                        ));
                                    } else {
                                        selected_spans.push(tui::text::Span::raw(segment.to_string()));
                                    }
                                }
                                segment_start = i;
                                current_selected = is_selected;
                            }
                        }
                        
                        // Add the final segment
                        if segment_start < after_text.len() {
                            let segment = &after_text[segment_start..];
                            if current_selected {
                                selected_spans.push(tui::text::Span::styled(
                                    segment.to_string(),
                                    Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD)
                                ));
                            } else {
                                selected_spans.push(tui::text::Span::raw(segment.to_string()));
                            }
                        }
                        
                        content_spans.extend(selected_spans);
                    }
                    
                    // Add all spans to the line
                    spans.extend(content_spans);
                } else if !content.is_empty() {
                    // No diagnostics but might be syntax highlighting
                    add_syntax_or_selection_spans(&mut spans, editor, tab, current_line, &content, left_column);
                }
            } else if !content.is_empty() {
                // No diagnostics but might be syntax highlighting
                add_syntax_or_selection_spans(&mut spans, editor, tab, current_line, &content, left_column);
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
    let cursor_x = tab.cursor.x.saturating_sub(left_column);
    let cursor_y = tab.cursor.y.saturating_sub(viewport.top_line);
    
    // Adjust cursor position for line numbers
    let line_number_offset = line_num_width + 1; // width + space
    
    f.set_cursor(
        area.x + cursor_x as u16 + line_number_offset as u16 + 1, // +1 for the border
        area.y + cursor_y as u16 + 1, // +1 for the border
    );
    
    // Return viewport dimensions for safe update
    Some(ViewportUpdate {
        width: viewport.width,
        height: viewport.height,
    })
}

/// Helper function to add either syntax highlighted spans or selection spans
fn add_syntax_or_selection_spans(spans: &mut Vec<tui::text::Span<'static>>, 
                              editor: &Editor, 
                              tab: &Tab, 
                              current_line: usize, 
                              content: &str, 
                              left_column: usize) {
    // First check if we have syntax highlighting
    if let Some(syntax_ref) = &tab.buffer.syntax {
        // Get line for highlighting
        let line_for_highlight = if current_line < tab.buffer.lines.len() {
            &tab.buffer.lines[current_line]
        } else {
            ""
        };
        
        // Use the cache to avoid recomputing syntax highlights
        let tab_idx = editor.current_tab;
        let cache_key = (tab_idx, current_line);
        
        // Get highlighted line from cache if available, or compute it
        let highlighted = if let Some(cached) = editor.highlighted_lines_cache.get(&cache_key) {
            cached.clone()
        } else {
            // Highlight the line (don't try to update cache since we have immutable reference)
            editor.syntax_highlighter.highlight_text(
                &format!("{}\n", line_for_highlight), 
                syntax_ref.clone()
            )
        };
        
        if !highlighted.is_empty() {
            // Process syntax highlighting with selection overlay
            let mut line_spans = Vec::new();
            
            for (style, text) in &highlighted[0].ranges {
                if text.is_empty() {
                    continue;
                }
                
                // Convert syntect style to tui style
                let tui_style = convert_syntect_style(style);
                
                // For each syntax span, we need to check if any part is selected
                // We can't directly calculate the offset using pointer arithmetic
                // because the strings might be in different memory locations
                // Instead, we'll use the start index from the syntax highlighting
                let rel_start = if let Some(idx) = line_for_highlight.find(text) {
                    idx
                } else {
                    // If we can't find the text in the line, use a safe default
                    0
                };
                
                if rel_start >= left_column {
                    let rel_text = &text[..];
                    let start_col = rel_start;
                    
                    // Check if any part of this span is selected
                    let mut current_selected = false;
                    let mut segment_start = 0;
                    let mut segments = Vec::new();
                    
                    // Iterate through characters and check selection status
                    for (i, _) in rel_text.char_indices() {
                        let col = start_col + i;
                        let is_selected = tab.buffer.is_position_selected(
                            current_line, 
                            col, 
                            &tab.cursor, 
                            tab.buffer.selection_start.is_some() && editor.mode == crate::editor::Mode::VisualLine
                        );
                        
                        if is_selected != current_selected {
                            // Transition between selected/unselected
                            if segment_start < i {
                                let segment = &rel_text[segment_start..i];
                                if current_selected {
                                    // Selected - use base style but with selection background
                                    segments.push(tui::text::Span::styled(
                                        segment.to_string(),
                                        tui_style.patch(Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD))
                                    ));
                                } else {
                                    // Not selected - use base syntax style
                                    segments.push(tui::text::Span::styled(
                                        segment.to_string(),
                                        tui_style
                                    ));
                                }
                            }
                            segment_start = i;
                            current_selected = is_selected;
                        }
                    }
                    
                    // Add the final segment
                    if segment_start < rel_text.len() {
                        let segment = &rel_text[segment_start..];
                        if current_selected {
                            segments.push(tui::text::Span::styled(
                                segment.to_string(),
                                tui_style.patch(Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD))
                            ));
                        } else {
                            segments.push(tui::text::Span::styled(
                                segment.to_string(),
                                tui_style
                            ));
                        }
                    }
                    
                    line_spans.extend(segments);
                }
            }
            
            if !line_spans.is_empty() {
                spans.extend(line_spans);
            } else {
                add_selection_only_spans(spans, tab, current_line, content, left_column, editor.mode == crate::editor::Mode::VisualLine);
            }
        } else {
            add_selection_only_spans(spans, tab, current_line, content, left_column, editor.mode == crate::editor::Mode::VisualLine);
        }
    } else {
        // No syntax highlighting, just add selection spans
        add_selection_only_spans(spans, tab, current_line, content, left_column, editor.mode == crate::editor::Mode::VisualLine);
    }
}

/// Helper function to add only selection spans when no syntax highlighting is available
fn add_selection_only_spans(spans: &mut Vec<tui::text::Span<'static>>,
                           tab: &Tab,
                           current_line: usize,
                           content: &str,
                           left_column: usize,
                           is_visual_line_mode: bool) {
    // Process the content character by character to handle selections
    let mut current_selected = false;
    let mut segment_start = 0;
    let mut segments = Vec::new();
    
    // Check each character if it's selected
    for (i, _) in content.char_indices() {
        let col = left_column + i;
        let is_selected = tab.buffer.is_position_selected(
            current_line, 
            col, 
            &tab.cursor, 
            tab.buffer.selection_start.is_some() && is_visual_line_mode
        );
        
        if is_selected != current_selected {
            // Transition between selected/unselected
            if segment_start < i {
                let segment = &content[segment_start..i];
                if current_selected {
                    segments.push(tui::text::Span::styled(
                        segment.to_string(),
                        Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD)
                    ));
                } else {
                    segments.push(tui::text::Span::raw(segment.to_string()));
                }
            }
            segment_start = i;
            current_selected = is_selected;
        }
    }
    
    // Add the final segment
    if segment_start < content.len() {
        let segment = &content[segment_start..];
        if current_selected {
            segments.push(tui::text::Span::styled(
                segment.to_string(),
                Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD)
            ));
        } else {
            segments.push(tui::text::Span::raw(segment.to_string()));
        }
    }
    
    spans.extend(segments);
}

fn render_filename_prompt<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    // Create a centered box for the filename prompt
    let prompt_area = centered_rect(60, 20, area);
    
    let prompt_block = Block::default()
        .title(" Enter Filename ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    
    let inner_area = prompt_block.inner(prompt_area);
    f.render_widget(prompt_block, prompt_area);
    
    // Render prompt text and input field
    let mut content = vec![
        Line::from(vec![
            tui::text::Span::raw("")
        ]),
        Line::from(vec![
            tui::text::Span::styled(
                "Please enter a filename to save:",
                Style::default().add_modifier(Modifier::BOLD)
            )
        ]),
        Line::from(vec![
            tui::text::Span::raw("")
        ]),
        Line::from(vec![
            tui::text::Span::styled(
                format!("> {}", editor.filename_prompt_text),
                Style::default().fg(Color::Green)
            )
        ]),
        Line::from(vec![
            tui::text::Span::raw("")
        ]),
        Line::from(vec![
            tui::text::Span::styled(
                "Press Enter to save, Esc to cancel",
                Style::default().fg(Color::DarkGray)
            )
        ]),
    ];
    
    // Add message if we're saving and quitting
    if editor.save_and_quit {
        content.push(Line::from(vec![
            tui::text::Span::raw("")
        ]));
        content.push(Line::from(vec![
            tui::text::Span::styled(
                "Editor will exit after saving",
                Style::default().fg(Color::Yellow)
            )
        ]));
    }
    
    let prompt_text = Paragraph::new(content)
        .alignment(tui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });
    
    f.render_widget(prompt_text, inner_area);
    
    // Position cursor at the end of the input field
    let prompt_prefix_len = 2; // "> " is 2 chars
    let cursor_pos_x = inner_area.x + (inner_area.width - prompt_prefix_len - editor.filename_prompt_text.len() as u16) / 2 
                      + prompt_prefix_len + editor.filename_prompt_text.len() as u16;
    let cursor_pos_y = inner_area.y + 3; // Position at the input line
    
    f.set_cursor(cursor_pos_x, cursor_pos_y);
}

/// Convert syntect style to tui style
/// 
/// This function converts a style from the syntect library into a style 
/// compatible with tui-rs for rendering in the terminal.
fn convert_syntect_style(style: &SyntectStyle) -> Style {
    let fg_color = style.foreground;
    
    // Convert RGB to a tui Color
    let r = fg_color.r;
    let g = fg_color.g;
    let b = fg_color.b;
    
    let fg = if r == 0 && g == 0 && b == 0 {
        // Default color
        Color::Reset
    } else {
        Color::Rgb(r, g, b)
    };
    
    let mut tui_style = Style::default().fg(fg);
    
    // Add font styling if applicable
    if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
        tui_style = tui_style.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
        tui_style = tui_style.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        tui_style = tui_style.add_modifier(Modifier::UNDERLINED);
    }
    
    tui_style
}

/// Creates line spans from highlighted text
/// 
/// This helper function extracts the common pattern of converting highlighted text
/// into tui-compatible spans, improving code maintainability.
fn create_highlighted_spans(highlighted: &[HighlightedLine]) -> Vec<Span<'static>> {
    if highlighted.is_empty() {
        return vec![];
    }
    
    highlighted[0].ranges.iter()
        .filter_map(|(style, text)| {
            // Skip empty text
            if text.is_empty() { 
                return None; 
            }
            
            // Convert syntect style to tui style
            let tui_style = convert_syntect_style(style);
            
            // Create the span with cloned text to avoid reference issues
            Some(Span::styled(text.clone(), tui_style))
        })
        .collect()
}

// Helper function to create a centered rect using percentage of the available space
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ].as_ref())
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ].as_ref())
        .split(popup_layout[1])[1]
}

fn render_file_finder<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    // Create a block for the file finder with a nicer title
    let file_finder_block = Block::default()
        .title(" Zim Editor ")
        .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = file_finder_block.inner(area);
    f.render_widget(file_finder_block, area);

    // Create overall layout with welcome header, search input, and file list
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Welcome header
            Constraint::Length(3), // Search input
            Constraint::Min(1),    // File list
        ].as_ref())
        .split(inner_area);

    // Render welcome header only if query is empty (initial state)
    if editor.file_finder.query().is_empty() {
        let welcome_text = vec![
            Line::from(vec![
                Span::styled("Welcome to ", Style::default().fg(Color::White)),
                Span::styled("ZIM Editor", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Type to search for files or use arrow keys to navigate", 
                    Style::default().fg(Color::Gray)),
            ]),
        ];
        
        let welcome_paragraph = Paragraph::new(welcome_text)
            .alignment(tui::layout::Alignment::Center)
            .block(Block::default()
                .borders(Borders::NONE));
        
        f.render_widget(welcome_paragraph, main_layout[0]);
    }

    // Render search query
    let search_block = Block::default()
        .title(" Search Files ")
        .title_style(Style::default().fg(Color::LightBlue))
        .borders(Borders::ALL);
    
    let search_text = Paragraph::new(editor.file_finder.query())
        .block(search_block)
        .style(Style::default());
    
    f.render_widget(search_text, main_layout[1]);

    // Render file list
    let list_title = if editor.file_finder.query().is_empty() {
        " Recent Files "
    } else {
        " Search Results "
    };
    
    let list_block = Block::default()
        .title(list_title)
        .title_style(Style::default().fg(Color::Green))
        .borders(Borders::ALL);

    let matches = editor.file_finder.matches();
    let selected_index = editor.file_finder.selected_index();
    
    let items: Vec<ListItem> = if matches.is_empty() && editor.file_finder.query().is_empty() {
        // Show a message when there are no recent files
        vec![ListItem::new("No recent files. Type to search or press Esc to open a blank file.")]
    } else if matches.is_empty() {
        // Show a message when there are no search results
        vec![ListItem::new("No matching files found. Press Esc to cancel.")]
    } else {
        matches
            .iter()
            .enumerate()
            .map(|(i, (path, _score))| {
                // Extract just the filename for display
                let display_path = if let Some(file_name) = std::path::Path::new(path).file_name() {
                    format!("{} ({})", file_name.to_string_lossy(), path)
                } else {
                    path.clone()
                };
                
                let style = if i == selected_index {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                
                ListItem::new(display_path).style(style)
            })
            .collect()
    };

    let file_list = List::new(items)
        .block(list_block)
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    
    f.render_widget(file_list, main_layout[2]);

    // Add a small help text at the bottom of the file list area
    let help_text = "Enter: open in current tab, Ctrl+Enter: open in new tab, Esc: normal mode, Ctrl+n: new file";
    let help_paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(tui::layout::Alignment::Center);
    
    // Calculate a small area at the bottom for the help text
    let help_area = Rect {
        x: main_layout[2].x,
        y: main_layout[2].y + main_layout[2].height.saturating_sub(1),
        width: main_layout[2].width,
        height: 1,
    };
    
    f.render_widget(help_paragraph, help_area);

    // Set cursor at the end of the search query
    f.set_cursor(
        main_layout[1].x + editor.file_finder.query().len() as u16 + 1,
        main_layout[1].y + 1,
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
            "╔══════════════════════════════════════╗",
            Style::default().fg(Color::Yellow)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "║       ZIM EDITOR HELP GUIDE          ║",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "╚══════════════════════════════════════╝",
            Style::default().fg(Color::Yellow)
        )
    ]));
    text.push(Line::from(""));
    
    // Normal Mode section
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "┏━━━━━━━━━━━━━━━━━━━━━┓",
            Style::default().fg(Color::Green)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "┃     NORMAL MODE     ┃",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "┗━━━━━━━━━━━━━━━━━━━━━┛",
            Style::default().fg(Color::Green)
        )
    ]));
    text.push(Line::from(""));
    
    // Basic navigation
    text.push(Line::from(vec![
        tui::text::Span::styled("➤ Basic Navigation:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("h, j, k, l - Move left, down, up, right"));
    text.push(Line::from("^ - Move to start of line"));
    text.push(Line::from("$ - Move to end of line"));
    text.push(Line::from("g - Move to top of file"));
    text.push(Line::from("G - Move to bottom of file"));
    text.push(Line::from("Ctrl+b - Page up"));
    text.push(Line::from("Ctrl+f - Page down"));
    text.push(Line::from(""));
    
    // Editing
    text.push(Line::from(vec![
        tui::text::Span::styled("➤ Editing:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("d - Delete current line"));
    text.push(Line::from("x - Delete character at cursor and enter insert mode"));
    text.push(Line::from("Backspace - In insert mode at line start, joins with previous line"));
    text.push(Line::from(""));
    
    // Modes
    text.push(Line::from(vec![
        tui::text::Span::styled("➤ Mode Switching:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("i - Enter Insert mode"));
    text.push(Line::from(": - Enter Command mode"));
    text.push(Line::from("ESC - Return to Normal mode (from any mode)"));
    text.push(Line::from(""));
    
    // Tabs
    text.push(Line::from(vec![
        tui::text::Span::styled("➤ Tab Management:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+n - New tab"));
    text.push(Line::from("Ctrl+w - Close tab"));
    text.push(Line::from("Ctrl+right - Next tab"));
    text.push(Line::from("Ctrl+left - Previous tab"));
    text.push(Line::from("F1-F12 - Switch directly to tabs 1-12"));
    text.push(Line::from(""));
    
    // File operations
    text.push(Line::from(vec![
        tui::text::Span::styled("➤ File Operations:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+p - Find file"));
    text.push(Line::from("q - Quit (in normal mode)"));
    text.push(Line::from(""));
    
    // File Actions
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
            Style::default().fg(Color::Cyan)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "┃    DIRECT COMMANDS (Normal Mode)       ┃",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            Style::default().fg(Color::Cyan)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled("w", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        tui::text::Span::raw("         - Save current file (prompts for confirmation, highlights changes)")
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled("e", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        tui::text::Span::raw("         - Reload current file from disk (prompts with diff highlighting)")
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled("X or ZZ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        tui::text::Span::raw("    - Save and quit (prompts for confirmation)")
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled("q", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        tui::text::Span::raw("         - Quit")
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(":q!", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        tui::text::Span::raw("       - Force quit (discard changes)")
    ]));
    text.push(Line::from(""));
    
    // Cargo integration
    text.push(Line::from(vec![
        tui::text::Span::styled("➤ Cargo Integration:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+d - Run cargo check"));
    text.push(Line::from("Ctrl+y - Run cargo clippy"));
    text.push(Line::from(""));
    
    // Help
    text.push(Line::from(vec![
        tui::text::Span::styled("➤ Help:", Style::default().add_modifier(Modifier::BOLD))
    ]));
    text.push(Line::from("Ctrl+h - Show this help page"));
    text.push(Line::from("ESC or q - Exit help and return to normal mode"));
    
    // Render the help text
    // Add footer
    text.push(Line::from(""));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "════════════════════════════════════════",
            Style::default().fg(Color::Yellow)
        )
    ]));
    text.push(Line::from(vec![
        tui::text::Span::styled(
            "   Zim: A modern, fast text editor      ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)
        )
    ]));
    
    let help_text = Paragraph::new(text)
        .alignment(tui::layout::Alignment::Left)
        .scroll((0, 0));
    
    f.render_widget(help_text, inner_area);
}

fn render_status_line<B: Backend>(f: &mut Frame<B>, editor: &Editor, area: Rect) {
    // Mode text with command text if in command mode
    let mode_text = match editor.mode {
        Mode::Normal => "NORMAL".to_string(),
        Mode::Insert => "INSERT".to_string(),
        Mode::Command => {
            format!(":{}", editor.command_text)
        },
        Mode::FileFinder => "FILE FINDER".to_string(),
        Mode::Help => "HELP".to_string(),
        Mode::WriteConfirm => "WRITE? (y/n/q)".to_string(),
        Mode::ReloadConfirm => "RELOAD? (y/n)".to_string(),
        Mode::FilenamePrompt => format!("FILENAME: {}", editor.filename_prompt_text),
        Mode::Visual => "VISUAL".to_string(),
        Mode::VisualLine => "VISUAL LINE".to_string(),
    };
    
    let status = match editor.mode {
        Mode::FileFinder => format!("{} | Press Enter to select, Esc to cancel", mode_text),
        Mode::FilenamePrompt => format!("{} | Press Enter to save, Esc to cancel", mode_text),
        Mode::WriteConfirm => {
            // Get current file info for write confirmation
            let file_info = if let Some(path) = &editor.current_tab().buffer.file_path {
                if path.starts_with("untitled-") {
                    "No filename specified".to_string()
                } else {
                    path.clone()
                }
            } else {
                "No filename specified".to_string()
            };
            
            // Count modified lines
            let modified_line_count = editor.current_tab().buffer.get_modified_lines().len();
            
            format!("{} | Save file: {} | {} modified lines | Press Y to confirm, N to cancel, Q to quit without saving", 
                mode_text, file_info, modified_line_count)
        },
        Mode::ReloadConfirm => {
            // Get current file info for reload confirmation
            let file_info = if let Some(path) = &editor.current_tab().buffer.file_path {
                path.clone()
            } else {
                "No filename specified".to_string()
            };
            
            // Count diff lines
            let diff_line_count = editor.diff_lines.len();
            
            format!("{} | Reload file: {} | {} changed lines | Press Y to confirm, N to cancel", 
                mode_text, file_info, diff_line_count)
        },
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
        .style(Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD));
    
    f.render_widget(status_bar, area);
}