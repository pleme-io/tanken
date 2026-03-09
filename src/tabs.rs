//! Tab management for multiple directory views.
//!
//! Each tab holds a `DualPane` (or single pane) with its own state.
//! Tabs can be created, closed, renamed, and switched between.

use std::path::PathBuf;

use crate::config::TankenConfig;
use crate::pane::DualPane;

/// A single tab with a name and dual-pane state.
#[derive(Debug)]
pub struct Tab {
    /// Display name (defaults to directory name).
    pub name: String,
    /// Dual-pane state.
    pub panes: DualPane,
}

impl Tab {
    /// Create a new tab for the given directory.
    pub fn new(path: PathBuf, config: &TankenConfig) -> Self {
        let name = path
            .file_name()
            .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned());

        let right_path = path.clone();
        Self {
            name,
            panes: DualPane::new(path, right_path, config),
        }
    }

    /// Update the tab name from the current active pane path.
    pub fn update_name(&mut self) {
        self.name = self
            .panes
            .active()
            .path
            .file_name()
            .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned());
    }
}

/// Tab manager: holds all open tabs and tracks the active one.
#[derive(Debug)]
pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl TabManager {
    /// Create a tab manager with one initial tab.
    pub fn new(initial_path: PathBuf, config: &TankenConfig) -> Self {
        Self {
            tabs: vec![Tab::new(initial_path, config)],
            active: 0,
        }
    }

    /// Get the active tab.
    #[must_use]
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// Get the active tab mutably.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Open a new tab at the given path and switch to it.
    pub fn open_tab(&mut self, path: PathBuf, config: &TankenConfig) {
        let tab = Tab::new(path, config);
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    /// Close the active tab. Returns false if it's the last tab.
    pub fn close_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.remove(self.active);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        true
    }

    /// Switch to the next tab.
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    /// Switch to the previous tab.
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    /// Switch to a specific tab index.
    pub fn go_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    /// Number of open tabs.
    #[must_use]
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_manager_basics() {
        let config = TankenConfig::default();
        let tmp = tempfile::TempDir::new().unwrap();
        let mut tm = TabManager::new(tmp.path().to_path_buf(), &config);

        assert_eq!(tm.count(), 1);
        assert_eq!(tm.active, 0);

        // Open a second tab
        tm.open_tab(tmp.path().to_path_buf(), &config);
        assert_eq!(tm.count(), 2);
        assert_eq!(tm.active, 1);

        // Switch tabs
        tm.next_tab();
        assert_eq!(tm.active, 0);
        tm.prev_tab();
        assert_eq!(tm.active, 1);

        // Close tab
        assert!(tm.close_tab());
        assert_eq!(tm.count(), 1);

        // Can't close last tab
        assert!(!tm.close_tab());
    }

    #[test]
    fn go_to_tab() {
        let config = TankenConfig::default();
        let tmp = tempfile::TempDir::new().unwrap();
        let mut tm = TabManager::new(tmp.path().to_path_buf(), &config);
        tm.open_tab(tmp.path().to_path_buf(), &config);
        tm.open_tab(tmp.path().to_path_buf(), &config);

        tm.go_to_tab(0);
        assert_eq!(tm.active, 0);
        tm.go_to_tab(2);
        assert_eq!(tm.active, 2);
        tm.go_to_tab(99); // out of bounds, no change
        assert_eq!(tm.active, 2);
    }
}
