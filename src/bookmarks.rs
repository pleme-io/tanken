//! Bookmark management: saved directories and recent locations.
//!
//! Bookmarks are persisted to `~/.config/tanken/bookmarks.json`.
//! Recent directories are tracked automatically with a configurable limit.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Maximum number of recent directories to track.
const MAX_RECENT: usize = 50;

/// Persisted bookmarks + recent directories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkStore {
    /// Manually bookmarked directories.
    pub bookmarks: Vec<PathBuf>,
    /// Recently visited directories (most recent first).
    pub recent: VecDeque<PathBuf>,
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self {
            bookmarks: Vec::new(),
            recent: VecDeque::new(),
        }
    }
}

impl BookmarkStore {
    /// Load bookmarks from disk.
    pub fn load() -> Self {
        let path = Self::store_path();
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Load bookmarks and merge with config defaults.
    pub fn load_with_defaults(config_bookmarks: &[String]) -> Self {
        let mut store = Self::load();

        // Add config bookmarks that aren't already in the list
        for bm in config_bookmarks {
            let expanded = expand_tilde(bm);
            if !store.bookmarks.contains(&expanded) {
                store.bookmarks.push(expanded);
            }
        }

        store
    }

    /// Save bookmarks to disk.
    pub fn save(&self) {
        let path = Self::store_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            fs::write(&path, data).ok();
        }
    }

    /// Add a bookmark. Returns true if it was added (not duplicate).
    pub fn add_bookmark(&mut self, path: PathBuf) -> bool {
        if self.bookmarks.contains(&path) {
            return false;
        }
        self.bookmarks.push(path);
        self.save();
        true
    }

    /// Remove a bookmark. Returns true if it was found and removed.
    pub fn remove_bookmark(&mut self, path: &Path) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|p| p != path);
        let removed = self.bookmarks.len() < before;
        if removed {
            self.save();
        }
        removed
    }

    /// Check if a path is bookmarked.
    #[must_use]
    pub fn is_bookmarked(&self, path: &Path) -> bool {
        self.bookmarks.iter().any(|p| p == path)
    }

    /// Record a directory visit (pushes to front of recent list).
    pub fn visit(&mut self, path: PathBuf) {
        // Remove if already in recent (we'll re-add at front)
        self.recent.retain(|p| p != &path);

        self.recent.push_front(path);

        // Trim to max
        while self.recent.len() > MAX_RECENT {
            self.recent.pop_back();
        }

        self.save();
    }

    /// Get the path to the bookmark store file.
    fn store_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tanken")
            .join("bookmarks.json")
    }
}

/// Expand `~` to the home directory.
#[must_use]
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_works() {
        let expanded = expand_tilde("~/Documents");
        assert!(!expanded.to_string_lossy().starts_with("~"));
        assert!(expanded.to_string_lossy().ends_with("Documents"));
    }

    #[test]
    fn expand_tilde_no_op() {
        let path = expand_tilde("/absolute/path");
        assert_eq!(path, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn bookmark_add_remove() {
        let mut store = BookmarkStore::default();
        let path = PathBuf::from("/tmp/test_bookmark");

        assert!(store.add_bookmark(path.clone()));
        assert!(!store.add_bookmark(path.clone())); // duplicate
        assert!(store.is_bookmarked(&path));
        assert!(store.remove_bookmark(&path));
        assert!(!store.is_bookmarked(&path));
    }

    #[test]
    fn recent_dirs_ordering() {
        let mut store = BookmarkStore::default();

        store.visit(PathBuf::from("/a"));
        store.visit(PathBuf::from("/b"));
        store.visit(PathBuf::from("/c"));

        assert_eq!(store.recent[0], PathBuf::from("/c"));
        assert_eq!(store.recent[1], PathBuf::from("/b"));
        assert_eq!(store.recent[2], PathBuf::from("/a"));

        // Re-visit /a → should move to front
        store.visit(PathBuf::from("/a"));
        assert_eq!(store.recent[0], PathBuf::from("/a"));
        assert_eq!(store.recent.len(), 3);
    }

    #[test]
    fn recent_dirs_max_limit() {
        let mut store = BookmarkStore::default();

        for i in 0..60 {
            store.visit(PathBuf::from(format!("/dir_{i}")));
        }

        assert!(store.recent.len() <= MAX_RECENT);
    }
}
