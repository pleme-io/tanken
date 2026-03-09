//! TUI rendering using ratatui + crossterm.
//!
//! Renders the dual-pane file manager with status bar, tab bar,
//! preview pane, and modal input overlays. Will be replaced by
//! garasu/madori GPU rendering in a future iteration.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::input::Mode;
use crate::pane::{self, Pane};
use crate::preview;

/// Main render function: draws the full UI.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Layout: tab bar | main area | status bar
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Min(5),   // main area
            Constraint::Length(1), // status bar
        ])
        .split(area);

    draw_tab_bar(frame, app, main_layout[0]);
    draw_main_area(frame, app, main_layout[1]);
    draw_status_bar(frame, app, main_layout[2]);

    // Draw input overlay if in a text mode
    if matches!(
        app.input.mode,
        Mode::Command | Mode::Search | Mode::Rename | Mode::Create { .. }
    ) {
        draw_input_overlay(frame, app, main_layout[2]);
    }
}

fn draw_tab_bar(frame: &mut Frame, app: &App, area: Rect) {
    let tabs = &app.tabs;
    let mut spans = Vec::new();

    for (i, tab) in tabs.tabs.iter().enumerate() {
        let style = if i == tabs.active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        spans.push(Span::styled(format!(" {} ", tab.name), style));
        spans.push(Span::raw(" "));
    }

    let line = Line::from(spans);
    let widget = Paragraph::new(line)
        .style(Style::default().bg(Color::DarkGray));
    frame.render_widget(widget, area);
}

fn draw_main_area(frame: &mut Frame, app: &mut App, area: Rect) {
    let tab = app.tabs.active_tab_mut();
    let dual = &mut tab.panes;

    // Split into left pane | right pane | preview
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ])
        .split(area);

    // Left pane
    let left_active = !dual.active_right;
    draw_file_list(frame, &mut dual.left, columns[0], left_active, "Left");

    // Right pane
    let right_active = dual.active_right;
    draw_file_list(frame, &mut dual.right, columns[1], right_active, "Right");

    // Preview pane
    let active_pane = if dual.active_right {
        &dual.right
    } else {
        &dual.left
    };
    draw_preview(frame, active_pane, columns[2]);
}

fn draw_file_list(
    frame: &mut Frame,
    pane: &mut Pane,
    area: Rect,
    is_active: bool,
    _label: &str,
) {
    let visible_height = area.height.saturating_sub(2) as usize; // minus borders
    pane.update_scroll(visible_height);

    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(
        " {} ",
        pane.path
            .file_name()
            .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned())
    );

    let items: Vec<ListItem> = pane
        .entries
        .iter()
        .enumerate()
        .skip(pane.scroll_offset)
        .take(visible_height)
        .map(|(i, entry)| {
            let is_selected = pane.selected.contains(&i);
            let is_cursor = i == pane.cursor;

            let icon = if entry.is_dir { "/" } else { " " };
            let size_str = if entry.is_dir {
                String::new()
            } else {
                pane::format_size(entry.size)
            };

            let date_str = pane::format_time(entry.modified);

            let line_text = format!(
                "{}{:<30} {:>8}  {}",
                icon,
                truncate_name(&entry.name, 29),
                size_str,
                date_str
            );

            let style = match (is_cursor, is_selected, entry.is_dir) {
                (true, true, _) => Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                (true, false, _) => Style::default()
                    .fg(Color::Black)
                    .bg(Color::White),
                (false, true, _) => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                (false, false, true) => Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                (false, false, false) => Style::default().fg(Color::White),
            };

            ListItem::new(Line::from(Span::styled(line_text, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title),
    );

    frame.render_widget(list, area);
}

fn draw_preview(frame: &mut Frame, active_pane: &Pane, area: Rect) {
    let preview_content = if let Some(entry) = active_pane.current_entry() {
        let pv = preview::generate_preview(&entry.path);
        preview::preview_to_lines(&pv)
    } else {
        vec!["No file selected".to_string()]
    };

    let text: Vec<Line> = preview_content
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                line.clone(),
                Style::default().fg(Color::Gray),
            ))
        })
        .collect();

    let preview_widget = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Preview "),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(preview_widget, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let tab = app.tabs.active_tab();
    let pane = tab.panes.active();

    let path_str = pane.path.display().to_string();
    let entry_count = pane.entries.len();
    let selected_count = pane.selected.len();
    let cursor_pos = if pane.entries.is_empty() {
        String::new()
    } else {
        format!("{}/{}", pane.cursor + 1, entry_count)
    };

    let mode_str = match app.input.mode {
        Mode::Normal => "NORMAL",
        Mode::Visual => "VISUAL",
        Mode::Command => "COMMAND",
        Mode::Search => "SEARCH",
        Mode::Rename => "RENAME",
        Mode::Create { is_dir: true } => "MKDIR",
        Mode::Create { is_dir: false } => "MKFILE",
    };

    let left = format!(" {mode_str} | {path_str}");
    let right = if selected_count > 0 {
        format!("{selected_count} selected | {cursor_pos} ")
    } else {
        format!("{cursor_pos} ")
    };

    let available = area.width as usize;
    let padding = available.saturating_sub(left.len() + right.len());

    let line = format!("{left}{}{right}", " ".repeat(padding));

    let widget = Paragraph::new(Line::from(Span::styled(
        line,
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray),
    )));

    frame.render_widget(widget, area);
}

fn draw_input_overlay(frame: &mut Frame, app: &App, area: Rect) {
    let prefix = match app.input.mode {
        Mode::Command => ":",
        Mode::Search => "/",
        Mode::Rename => "rename: ",
        Mode::Create { is_dir: true } => "mkdir: ",
        Mode::Create { is_dir: false } => "touch: ",
        _ => "",
    };

    let text = format!("{prefix}{}", app.input.input_buffer);
    let widget = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default()
            .fg(Color::Yellow)
            .bg(Color::Black)
            .add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(widget, area);
}

/// Truncate a name to fit a given width, adding ".." if needed.
fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else if max_len > 2 {
        format!("{}...", &name[..max_len - 3])
    } else {
        name[..max_len].to_string()
    }
}
