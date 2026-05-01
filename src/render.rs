//! TUI rendering using `egaku-term`.
//!
//! Layout: tab bar (1 row) | main area | status bar (1 row), with an
//! optional input-mode overlay painted on top of the status row.
//! Main area splits into three columns: left pane | right pane | preview.

use crossterm::style::{Attribute, Color};
use egaku::Rect;
use egaku_term::crossterm::{
    QueueableCommand,
    cursor::MoveTo,
    style::{Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
};
use egaku_term::{Terminal, draw, theme::Palette};

use crate::app::App;
use crate::input::Mode;
use crate::pane::{self, Pane};
use crate::preview;

/// Compact style triple — fg, optional bg, attribute (bold/reset).
#[derive(Clone, Copy)]
struct Style {
    fg: Color,
    bg: Option<Color>,
    attr: Attribute,
}

impl Style {
    const fn fg(c: Color) -> Self {
        Self { fg: c, bg: None, attr: Attribute::Reset }
    }
    const fn bg(self, c: Color) -> Self {
        Self { bg: Some(c), ..self }
    }
    const fn bold(self) -> Self {
        Self { attr: Attribute::Bold, ..self }
    }
}

/// Default file-manager palette — Nord-flavored constants chosen to
/// match the previous ratatui rendering.
const PAL_FG: Color = Color::Rgb { r: 216, g: 222, b: 233 };       // Nord4
const PAL_BG: Color = Color::Rgb { r: 46, g: 52, b: 64 };          // Nord0
const PAL_DIM: Color = Color::Rgb { r: 76, g: 86, b: 106 };        // Nord3
const PAL_BORDER: Color = Color::Rgb { r: 76, g: 86, b: 106 };     // Nord3
const PAL_BORDER_FOCUS: Color = Color::Rgb { r: 136, g: 192, b: 208 }; // Nord8
const PAL_DIR: Color = Color::Rgb { r: 129, g: 161, b: 193 };      // Nord9 (blue)
const PAL_CURSOR_BG: Color = Color::Rgb { r: 229, g: 233, b: 240 };  // Nord5
const PAL_CURSOR_FG: Color = Color::Rgb { r: 46, g: 52, b: 64 };     // Nord0
const PAL_SELECTED_FG: Color = Color::Rgb { r: 235, g: 203, b: 139 };  // Nord13 (yellow)
const PAL_SELECTED_BG: Color = Color::Rgb { r: 235, g: 203, b: 139 };  // also yellow for cursor+selected
const PAL_INPUT_FG: Color = Color::Rgb { r: 235, g: 203, b: 139 };   // Nord13
const PAL_TAB_BAR_BG: Color = Color::Rgb { r: 59, g: 66, b: 82 };    // Nord1
const PAL_TAB_ACTIVE_FG: Color = Color::Rgb { r: 46, g: 52, b: 64 }; // Nord0
const PAL_TAB_ACTIVE_BG: Color = Color::Rgb { r: 136, g: 192, b: 208 }; // Nord8
const PAL_TAB_INACTIVE_FG: Color = Color::Rgb { r: 200, g: 200, b: 200 };
const PAL_STATUS_BG: Color = Color::Rgb { r: 59, g: 66, b: 82 };
const PAL_STATUS_FG: Color = Color::Rgb { r: 216, g: 222, b: 233 };
const PAL_INPUT_BG: Color = PAL_BG;

fn palette() -> Palette {
    Palette {
        background: PAL_BG,
        foreground: PAL_FG,
        accent: PAL_BORDER_FOCUS,
        error: Color::Rgb { r: 191, g: 97, b: 106 },
        warning: Color::Rgb { r: 235, g: 203, b: 139 },
        success: Color::Rgb { r: 163, g: 190, b: 140 },
        selection: PAL_DIM,
        muted: PAL_DIM,
        border: PAL_BORDER,
    }
}

/// Main render entry: clears terminal, paints frame, flushes.
pub fn draw(term: &mut Terminal, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = term.size().map_err(map_err)?;
    if cols == 0 || rows < 3 {
        return Ok(());
    }
    let cols_f = f32::from(cols);
    let rows_f = f32::from(rows);

    fill_bg(term, cols, rows)?;

    let tab_rect = Rect::new(0.0, 0.0, cols_f, 1.0);
    let main_rect = Rect::new(0.0, 1.0, cols_f, rows_f - 2.0);
    let status_rect = Rect::new(0.0, rows_f - 1.0, cols_f, 1.0);

    draw_tab_bar(term, app, tab_rect)?;
    draw_main_area(term, app, main_rect)?;
    draw_status_bar(term, app, status_rect)?;

    if matches!(
        app.input.mode,
        Mode::Command | Mode::Search | Mode::Rename | Mode::Create { .. }
    ) {
        draw_input_overlay(term, app, status_rect)?;
    }
    Ok(())
}

fn fill_bg(term: &mut Terminal, cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error>> {
    let blank = " ".repeat(usize::from(cols));
    term.out()
        .queue(SetBackgroundColor(PAL_BG))?
        .queue(SetForegroundColor(PAL_FG))?;
    for r in 0..rows {
        term.out().queue(MoveTo(0, r))?.queue(Print(&blank))?;
    }
    term.out().queue(ResetColor)?;
    Ok(())
}

fn draw_tab_bar(term: &mut Terminal, app: &App, rect: Rect) -> Result<(), Box<dyn std::error::Error>> {
    let (x, y, w, _h) = cells(rect);
    if w == 0 {
        return Ok(());
    }
    let blank = " ".repeat(usize::from(w));
    term.out()
        .queue(SetBackgroundColor(PAL_TAB_BAR_BG))?
        .queue(MoveTo(x, y))?
        .queue(Print(&blank))?;

    let mut col: u16 = x;
    for (i, tab) in app.tabs.tabs.iter().enumerate() {
        let label = format!(" {} ", tab.name);
        let lw = u16::try_from(label.chars().count()).unwrap_or(w).min(w);
        if col + lw > x + w {
            break;
        }
        let style = if i == app.tabs.active {
            Style::fg(PAL_TAB_ACTIVE_FG).bg(PAL_TAB_ACTIVE_BG).bold()
        } else {
            Style::fg(PAL_TAB_INACTIVE_FG).bg(PAL_TAB_BAR_BG)
        };
        paint_styled(term, col, y, lw, &label, style)?;
        col += lw + 1;
    }
    term.out().queue(ResetColor)?;
    Ok(())
}

fn draw_main_area(term: &mut Terminal, app: &mut App, rect: Rect) -> Result<(), Box<dyn std::error::Error>> {
    let (x, y, w, h) = cells(rect);
    if w < 6 || h < 3 {
        return Ok(());
    }

    // Three columns: 30% / 35% / 35%
    let left_w = u16::try_from((u32::from(w) * 30) / 100).unwrap_or(0).max(8);
    let right_w = u16::try_from((u32::from(w) * 35) / 100).unwrap_or(0).max(8);
    let preview_w = w - left_w - right_w;

    let left_rect = Rect::new(f32::from(x), f32::from(y), f32::from(left_w), f32::from(h));
    let right_rect = Rect::new(
        f32::from(x + left_w),
        f32::from(y),
        f32::from(right_w),
        f32::from(h),
    );
    let preview_rect = Rect::new(
        f32::from(x + left_w + right_w),
        f32::from(y),
        f32::from(preview_w),
        f32::from(h),
    );

    let tab = app.tabs.active_tab_mut();
    let dual = &mut tab.panes;
    let left_active = !dual.active_right;
    let right_active = dual.active_right;

    draw_file_list(term, &mut dual.left, left_rect, left_active)?;
    draw_file_list(term, &mut dual.right, right_rect, right_active)?;

    let active_pane = if dual.active_right { &dual.right } else { &dual.left };
    draw_preview(term, active_pane, preview_rect)?;
    Ok(())
}

fn draw_file_list(
    term: &mut Terminal,
    pane: &mut Pane,
    rect: Rect,
    is_active: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let pal = palette();
    let title = format!(
        " {} ",
        pane.path
            .file_name()
            .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned())
    );
    draw::bordered_block_with(term, rect, &title, is_active, &pal).map_err(map_err)?;

    let inner = draw::block_inner(rect);
    let (ix, iy, iw, ih) = cells(inner);
    if iw == 0 || ih == 0 {
        return Ok(());
    }

    pane.update_scroll(usize::from(ih));

    for (rel_i, (i, entry)) in pane
        .entries
        .iter()
        .enumerate()
        .skip(pane.scroll_offset)
        .take(usize::from(ih))
        .enumerate()
    {
        let row = u16::try_from(rel_i).unwrap_or(u16::MAX);
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
            (true, true, _) => Style::fg(PAL_TAB_ACTIVE_FG).bg(PAL_SELECTED_BG).bold(),
            (true, false, _) => Style::fg(PAL_CURSOR_FG).bg(PAL_CURSOR_BG),
            (false, true, _) => Style::fg(PAL_SELECTED_FG).bold(),
            (false, false, true) => Style::fg(PAL_DIR).bold(),
            (false, false, false) => Style::fg(PAL_FG),
        };
        paint_styled(term, ix, iy + row, iw, &line_text, style)?;
    }
    Ok(())
}

fn draw_preview(term: &mut Terminal, active_pane: &Pane, rect: Rect) -> Result<(), Box<dyn std::error::Error>> {
    let pal = palette();
    draw::bordered_block_with(term, rect, " Preview ", false, &pal).map_err(map_err)?;
    let inner = draw::block_inner(rect);
    let (ix, iy, iw, ih) = cells(inner);
    if iw == 0 || ih == 0 {
        return Ok(());
    }

    let lines = if let Some(entry) = active_pane.current_entry() {
        let pv = preview::generate_preview(&entry.path);
        preview::preview_to_lines(&pv)
    } else {
        vec!["No file selected".to_string()]
    };

    for (i, line) in lines.iter().enumerate().take(usize::from(ih)) {
        let row = u16::try_from(i).unwrap_or(u16::MAX);
        paint_styled(term, ix, iy + row, iw, line, Style::fg(PAL_DIM))?;
    }
    Ok(())
}

fn draw_status_bar(term: &mut Terminal, app: &App, rect: Rect) -> Result<(), Box<dyn std::error::Error>> {
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

    let mut pal = palette();
    pal.background = PAL_STATUS_BG;
    pal.foreground = PAL_STATUS_FG;
    pal.selection = PAL_STATUS_BG;
    draw::status_line_with(term, rect, &left, &right, &pal).map_err(map_err)
}

fn draw_input_overlay(term: &mut Terminal, app: &App, rect: Rect) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = match app.input.mode {
        Mode::Command => ":",
        Mode::Search => "/",
        Mode::Rename => "rename: ",
        Mode::Create { is_dir: true } => "mkdir: ",
        Mode::Create { is_dir: false } => "touch: ",
        _ => "",
    };
    let text = format!("{prefix}{}", app.input.input_buffer);
    let (x, y, w, _h) = cells(rect);
    let blank = " ".repeat(usize::from(w));
    term.out()
        .queue(SetBackgroundColor(PAL_INPUT_BG))?
        .queue(MoveTo(x, y))?
        .queue(Print(&blank))?;
    paint_styled(
        term,
        x,
        y,
        w,
        &text,
        Style::fg(PAL_INPUT_FG).bg(PAL_INPUT_BG).bold(),
    )?;
    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn paint_styled(
    term: &mut Terminal,
    col: u16,
    row: u16,
    max: u16,
    text: &str,
    style: Style,
) -> Result<(), Box<dyn std::error::Error>> {
    if max == 0 {
        return Ok(());
    }
    let line: String = text.chars().take(usize::from(max)).collect();
    term.out()
        .queue(MoveTo(col, row))?
        .queue(SetForegroundColor(style.fg))?;
    if let Some(bg) = style.bg {
        term.out().queue(SetBackgroundColor(bg))?;
    }
    if !matches!(style.attr, Attribute::Reset) {
        term.out().queue(SetAttribute(style.attr))?;
    }
    term.out().queue(Print(line))?;
    if !matches!(style.attr, Attribute::Reset) {
        term.out().queue(SetAttribute(Attribute::Reset))?;
    }
    term.out().queue(ResetColor)?;
    Ok(())
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn cells(rect: Rect) -> (u16, u16, u16, u16) {
    let to_u16 = |f: f32| f.max(0.0).round().min(f32::from(u16::MAX)) as u16;
    (
        to_u16(rect.x),
        to_u16(rect.y),
        to_u16(rect.width),
        to_u16(rect.height),
    )
}

fn map_err(e: egaku_term::Error) -> Box<dyn std::error::Error> {
    Box::<dyn std::error::Error>::from(e.to_string())
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.chars().count() <= max_len {
        name.to_string()
    } else if max_len > 2 {
        let head: String = name.chars().take(max_len - 3).collect();
        format!("{head}...")
    } else {
        name.chars().take(max_len).collect()
    }
}
