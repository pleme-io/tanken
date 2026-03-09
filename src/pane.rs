//! Pane state: cursor position, selection, sorting, and directory listing.
//!
//! Each pane represents a view into a single directory. The dual-pane
//! and Miller-column layouts compose multiple panes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::config::TankenConfig;
use crate::platform::FileEntry;

/// Sort field for directory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Name,
    Size,
    Modified,
    Extension,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// A single pane displaying a directory.
#[derive(Debug)]
pub struct Pane {
    /// Current directory.
    pub path: PathBuf,
    /// All entries (sorted, filtered).
    pub entries: Vec<FileEntry>,
    /// Cursor index into `entries`.
    pub cursor: usize,
    /// Selected entry indices.
    pub selected: HashSet<usize>,
    /// Sort field.
    pub sort_field: SortField,
    /// Sort direction.
    pub sort_dir: SortDirection,
    /// Whether to list directories before files.
    pub dirs_first: bool,
    /// Whether to show hidden files.
    pub show_hidden: bool,
    /// Scroll offset for rendering.
    pub scroll_offset: usize,
    /// Filter string for incremental search.
    pub filter: String,
}

impl Pane {
    /// Create a new pane at the given directory.
    pub fn new(path: PathBuf, config: &TankenConfig) -> Self {
        let mut pane = Self {
            path: path.clone(),
            entries: Vec::new(),
            cursor: 0,
            selected: HashSet::new(),
            sort_field: SortField::Name,
            sort_dir: SortDirection::Ascending,
            dirs_first: true,
            show_hidden: config.appearance.show_hidden,
            scroll_offset: 0,
            filter: String::new(),
        };
        pane.refresh();
        pane
    }

    /// Reload the directory listing.
    pub fn refresh(&mut self) {
        self.entries = crate::fs::list_directory(&self.path, self.show_hidden)
            .unwrap_or_default();

        if !self.filter.is_empty() {
            let filter_lower = self.filter.to_lowercase();
            self.entries.retain(|e| {
                e.name.to_lowercase().contains(&filter_lower)
            });
        }

        self.sort_entries();
        self.clamp_cursor();
        self.selected.clear();
    }

    /// Navigate into the entry under cursor. Returns true if navigation happened.
    pub fn enter(&mut self) -> bool {
        if let Some(entry) = self.current_entry() {
            if entry.is_dir {
                let new_path = entry.path.clone();
                self.path = new_path;
                self.cursor = 0;
                self.scroll_offset = 0;
                self.filter.clear();
                self.refresh();
                return true;
            }
        }
        false
    }

    /// Navigate to the parent directory. Returns true if navigation happened.
    pub fn go_parent(&mut self) -> bool {
        if let Some(parent) = self.path.parent().map(Path::to_path_buf) {
            self.path = parent;
            self.cursor = 0;
            self.scroll_offset = 0;
            self.filter.clear();
            self.refresh();
            true
        } else {
            false
        }
    }

    /// Navigate to a specific directory.
    pub fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.path = path;
            self.cursor = 0;
            self.scroll_offset = 0;
            self.filter.clear();
            self.refresh();
        }
    }

    /// Move cursor down.
    pub fn cursor_down(&mut self) {
        if !self.entries.is_empty() && self.cursor < self.entries.len() - 1 {
            self.cursor += 1;
        }
    }

    /// Move cursor up.
    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Jump to the first entry.
    pub fn cursor_top(&mut self) {
        self.cursor = 0;
    }

    /// Jump to the last entry.
    pub fn cursor_bottom(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = self.entries.len() - 1;
        }
    }

    /// Toggle selection of the entry under cursor.
    pub fn toggle_selection(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        if self.selected.contains(&self.cursor) {
            self.selected.remove(&self.cursor);
        } else {
            self.selected.insert(self.cursor);
        }
    }

    /// Select all entries.
    pub fn select_all(&mut self) {
        for i in 0..self.entries.len() {
            self.selected.insert(i);
        }
    }

    /// Clear all selections.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Get the entry under the cursor.
    #[must_use]
    pub fn current_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.cursor)
    }

    /// Get all selected entries. If nothing is selected, returns the cursor entry.
    #[must_use]
    pub fn selected_entries(&self) -> Vec<&FileEntry> {
        if self.selected.is_empty() {
            self.current_entry().into_iter().collect()
        } else {
            self.selected
                .iter()
                .filter_map(|&i| self.entries.get(i))
                .collect()
        }
    }

    /// Get paths of all selected entries.
    #[must_use]
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        self.selected_entries()
            .iter()
            .map(|e| e.path.clone())
            .collect()
    }

    /// Set sort field and re-sort.
    pub fn set_sort(&mut self, field: SortField, direction: SortDirection) {
        self.sort_field = field;
        self.sort_dir = direction;
        self.sort_entries();
    }

    /// Toggle hidden file visibility.
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh();
    }

    /// Set the filter string and re-filter entries.
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.refresh();
    }

    /// Update scroll offset to keep cursor visible.
    pub fn update_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor - visible_height + 1;
        }
    }

    fn sort_entries(&mut self) {
        let dirs_first = self.dirs_first;
        let field = self.sort_field;
        let ascending = self.sort_dir == SortDirection::Ascending;

        self.entries.sort_by(|a, b| {
            // Directories first (if enabled)
            if dirs_first && a.is_dir != b.is_dir {
                return if a.is_dir {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }

            let cmp = match field {
                SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortField::Size => a.size.cmp(&b.size),
                SortField::Modified => {
                    a.modified.cmp(&b.modified)
                }
                SortField::Extension => {
                    let ext_a = Path::new(&a.name)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    let ext_b = Path::new(&b.name)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    ext_a.cmp(ext_b)
                }
            };

            if ascending { cmp } else { cmp.reverse() }
        });
    }

    fn clamp_cursor(&mut self) {
        if self.entries.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= self.entries.len() {
            self.cursor = self.entries.len() - 1;
        }
    }
}

/// Dual-pane state: left + right panes with active pane tracking.
#[derive(Debug)]
pub struct DualPane {
    pub left: Pane,
    pub right: Pane,
    /// Which pane is active: `false` = left, `true` = right.
    pub active_right: bool,
}

impl DualPane {
    /// Create a dual-pane layout.
    pub fn new(left_path: PathBuf, right_path: PathBuf, config: &TankenConfig) -> Self {
        Self {
            left: Pane::new(left_path, config),
            right: Pane::new(right_path, config),
            active_right: false,
        }
    }

    /// Get a reference to the active pane.
    #[must_use]
    pub fn active(&self) -> &Pane {
        if self.active_right { &self.right } else { &self.left }
    }

    /// Get a mutable reference to the active pane.
    pub fn active_mut(&mut self) -> &mut Pane {
        if self.active_right { &mut self.right } else { &mut self.left }
    }

    /// Get a reference to the inactive pane.
    #[must_use]
    pub fn inactive(&self) -> &Pane {
        if self.active_right { &self.left } else { &self.right }
    }

    /// Switch the active pane.
    pub fn toggle_active(&mut self) {
        self.active_right = !self.active_right;
    }
}

/// Format a `SystemTime` as a human-readable date string.
#[must_use]
pub fn format_time(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    let dt = chrono::DateTime::from_timestamp(
        i64::try_from(secs).unwrap_or(0),
        0,
    );
    match dt {
        Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        None => "unknown".to_string(),
    }
}

/// Format a file size in human-readable form.
#[must_use]
pub fn format_size(bytes: u64) -> String {
    humansize::format_size(bytes, humansize::BINARY)
}

/// Format Unix permissions from mode bits.
#[must_use]
pub fn format_permissions(path: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata() {
            let mode = meta.permissions().mode();
            let mut s = String::with_capacity(10);

            if path.is_dir() {
                s.push('d');
            } else if path.is_symlink() {
                s.push('l');
            } else {
                s.push('-');
            }

            // Owner
            s.push(if mode & 0o400 != 0 { 'r' } else { '-' });
            s.push(if mode & 0o200 != 0 { 'w' } else { '-' });
            s.push(if mode & 0o100 != 0 { 'x' } else { '-' });
            // Group
            s.push(if mode & 0o040 != 0 { 'r' } else { '-' });
            s.push(if mode & 0o020 != 0 { 'w' } else { '-' });
            s.push(if mode & 0o010 != 0 { 'x' } else { '-' });
            // Other
            s.push(if mode & 0o004 != 0 { 'r' } else { '-' });
            s.push(if mode & 0o002 != 0 { 'w' } else { '-' });
            s.push(if mode & 0o001 != 0 { 'x' } else { '-' });

            return s;
        }
        "----------".to_string()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        "----------".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_works() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1024), "1 KiB");
        assert_eq!(format_size(1_048_576), "1 MiB");
    }

    #[test]
    fn format_time_works() {
        let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let s = format_time(time);
        assert!(s.contains("2023"));
    }

    #[test]
    fn sort_entries_dirs_first() {
        let config = TankenConfig::default();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("b.txt"), "").unwrap();
        std::fs::create_dir(tmp.path().join("a_dir")).unwrap();
        std::fs::write(tmp.path().join("c.txt"), "").unwrap();

        let pane = Pane::new(tmp.path().to_path_buf(), &config);
        // Dirs should come first
        assert!(pane.entries[0].is_dir);
    }

    #[test]
    fn cursor_navigation() {
        let config = TankenConfig::default();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "").unwrap();
        std::fs::write(tmp.path().join("c.txt"), "").unwrap();

        let mut pane = Pane::new(tmp.path().to_path_buf(), &config);
        assert_eq!(pane.cursor, 0);
        pane.cursor_down();
        assert_eq!(pane.cursor, 1);
        pane.cursor_down();
        assert_eq!(pane.cursor, 2);
        pane.cursor_down(); // should not go past end
        assert_eq!(pane.cursor, 2);
        pane.cursor_up();
        assert_eq!(pane.cursor, 1);
        pane.cursor_top();
        assert_eq!(pane.cursor, 0);
        pane.cursor_bottom();
        assert_eq!(pane.cursor, 2);
    }

    #[test]
    fn toggle_selection() {
        let config = TankenConfig::default();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "").unwrap();

        let mut pane = Pane::new(tmp.path().to_path_buf(), &config);
        assert!(pane.selected.is_empty());
        pane.toggle_selection();
        assert!(pane.selected.contains(&0));
        pane.toggle_selection();
        assert!(pane.selected.is_empty());
    }
}
