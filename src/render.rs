//! TUI rendering using `egaku-term`.
//!
//! Layout: tab bar (1 row) | main area | status bar (1 row), with an
//! optional input-mode overlay painted on top of the status row.
//! Main area splits into three columns: left pane | right pane | preview.

use std::sync::LazyLock;

use crossterm::style::{Attribute, Color};
use egaku::Rect;
use egaku_term::crossterm::{
    QueueableCommand,
    cursor::MoveTo,
    style::{Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
};
use egaku_term::{Terminal, draw, theme::Palette};
use ishou_tokens::{ColorPalette, Rgb};

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
        Self {
            fg: c,
            bg: None,
            attr: Attribute::Reset,
        }
    }
    const fn bg(self, c: Color) -> Self {
        Self {
            bg: Some(c),
            ..self
        }
    }
    const fn bold(self) -> Self {
        Self {
            attr: Attribute::Bold,
            ..self
        }
    }
}

/// A crossterm paint colour from an ishou Nord token.
const fn nord(c: Rgb) -> Color {
    Color::Rgb {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

/// Tanken's file-manager paint palette, sourced entirely from ishou's
/// Nord tokens (`ColorPalette::pleme()`) — no hand-authored hex at any
/// paint site. Each field is annotated with its Nord index for parity
/// with the design system; the values are byte-identical to the Nord
/// palette ishou ships, so the palette can never drift from the fleet
/// design framework.
struct TankenPalette {
    fg: Color,
    bg: Color,
    dim: Color,
    border: Color,
    border_focus: Color,
    dir: Color,
    cursor_bg: Color,
    cursor_fg: Color,
    selected_fg: Color,
    selected_bg: Color,
    input_fg: Color,
    tab_bar_bg: Color,
    tab_active_fg: Color,
    tab_active_bg: Color,
    tab_inactive_fg: Color,
    status_bg: Color,
    status_fg: Color,
    input_bg: Color,
    error: Color,
    warning: Color,
    success: Color,
}

/// Resolved once at first paint from the fleet Nord tokens.
static PAL: LazyLock<TankenPalette> = LazyLock::new(|| {
    let n = ColorPalette::pleme();
    TankenPalette {
        fg: nord(n.snow_storm_0),             // Nord4
        bg: nord(n.polar_night_0),            // Nord0
        dim: nord(n.polar_night_3),           // Nord3
        border: nord(n.polar_night_3),        // Nord3
        border_focus: nord(n.frost_1),        // Nord8
        dir: nord(n.frost_2),                 // Nord9 (blue)
        cursor_bg: nord(n.snow_storm_1),      // Nord5
        cursor_fg: nord(n.polar_night_0),     // Nord0
        selected_fg: nord(n.aurora_yellow),   // Nord13 (yellow)
        selected_bg: nord(n.aurora_yellow),   // Nord13 — cursor+selected
        input_fg: nord(n.aurora_yellow),      // Nord13
        tab_bar_bg: nord(n.polar_night_1),    // Nord1
        tab_active_fg: nord(n.polar_night_0), // Nord0
        tab_active_bg: nord(n.frost_1),       // Nord8
        // Was a lone generic grey (200,200,200) — now a real Nord token,
        // restoring palette consistency. Snow Storm reads clearly on the
        // dark tab bar while staying de-emphasised vs the active tab.
        tab_inactive_fg: nord(n.snow_storm_0), // Nord4
        status_bg: nord(n.polar_night_1),      // Nord1
        status_fg: nord(n.snow_storm_0),       // Nord4
        input_bg: nord(n.polar_night_0),       // Nord0
        error: nord(n.aurora_red),             // Nord11
        warning: nord(n.aurora_yellow),        // Nord13
        success: nord(n.aurora_green),         // Nord14
    }
});

fn palette() -> Palette {
    Palette {
        background: PAL.bg,
        foreground: PAL.fg,
        accent: PAL.border_focus,
        error: PAL.error,
        warning: PAL.warning,
        success: PAL.success,
        selection: PAL.dim,
        muted: PAL.dim,
        border: PAL.border,
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
        .queue(SetBackgroundColor(PAL.bg))?
        .queue(SetForegroundColor(PAL.fg))?;
    for r in 0..rows {
        term.out().queue(MoveTo(0, r))?.queue(Print(&blank))?;
    }
    term.out().queue(ResetColor)?;
    Ok(())
}

fn draw_tab_bar(
    term: &mut Terminal,
    app: &App,
    rect: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
    let (x, y, w, _h) = cells(rect);
    if w == 0 {
        return Ok(());
    }
    let blank = " ".repeat(usize::from(w));
    term.out()
        .queue(SetBackgroundColor(PAL.tab_bar_bg))?
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
            Style::fg(PAL.tab_active_fg).bg(PAL.tab_active_bg).bold()
        } else {
            Style::fg(PAL.tab_inactive_fg).bg(PAL.tab_bar_bg)
        };
        paint_styled(term, col, y, lw, &label, style)?;
        col += lw + 1;
    }
    term.out().queue(ResetColor)?;
    Ok(())
}

fn draw_main_area(
    term: &mut Terminal,
    app: &mut App,
    rect: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let active_pane = if dual.active_right {
        &dual.right
    } else {
        &dual.left
    };
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
            (true, true, _) => Style::fg(PAL.tab_active_fg).bg(PAL.selected_bg).bold(),
            (true, false, _) => Style::fg(PAL.cursor_fg).bg(PAL.cursor_bg),
            (false, true, _) => Style::fg(PAL.selected_fg).bold(),
            (false, false, true) => Style::fg(PAL.dir).bold(),
            (false, false, false) => Style::fg(PAL.fg),
        };
        paint_styled(term, ix, iy + row, iw, &line_text, style)?;
    }
    Ok(())
}

fn draw_preview(
    term: &mut Terminal,
    active_pane: &Pane,
    rect: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
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
        paint_styled(term, ix, iy + row, iw, line, Style::fg(PAL.dim))?;
    }
    Ok(())
}

fn draw_status_bar(
    term: &mut Terminal,
    app: &App,
    rect: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
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
    pal.background = PAL.status_bg;
    pal.foreground = PAL.status_fg;
    pal.selection = PAL.status_bg;
    draw::status_line_with(term, rect, &left, &right, &pal).map_err(map_err)
}

fn draw_input_overlay(
    term: &mut Terminal,
    app: &App,
    rect: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
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
        .queue(SetBackgroundColor(PAL.input_bg))?
        .queue(MoveTo(x, y))?
        .queue(Print(&blank))?;
    paint_styled(
        term,
        x,
        y,
        w,
        &text,
        Style::fg(PAL.input_fg).bg(PAL.input_bg).bold(),
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
