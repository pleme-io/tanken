//! Main application state and event loop.
//!
//! The `App` struct holds all state (tabs, input, clipboard, bookmarks)
//! and processes actions from the input handler into state changes.

use std::path::PathBuf;

use crate::bookmarks::BookmarkStore;
use crate::config::TankenConfig;
use crate::input::{Action, InputHandler, Mode};
use crate::pane::{SortDirection, SortField};
use crate::search::SearchEngine;
use crate::tabs::TabManager;

/// Clipboard state for yank/cut operations.
#[derive(Debug, Clone)]
pub struct Clipboard {
    /// Paths that have been yanked or cut.
    pub paths: Vec<PathBuf>,
    /// Whether this is a cut (move) or yank (copy).
    pub is_cut: bool,
}

/// Main application state.
pub struct App {
    pub config: TankenConfig,
    pub tabs: TabManager,
    pub input: InputHandler,
    pub bookmarks: BookmarkStore,
    pub clipboard: Option<Clipboard>,
    pub search: SearchEngine,
    pub message: Option<String>,
    pub should_quit: bool,
}

impl App {
    /// Create a new application.
    pub fn new(config: TankenConfig, initial_path: PathBuf) -> Self {
        let bookmarks =
            BookmarkStore::load_with_defaults(&config.navigation.bookmarks);

        Self {
            tabs: TabManager::new(initial_path.clone(), &config),
            input: InputHandler::new(),
            bookmarks,
            clipboard: None,
            search: SearchEngine::new(),
            message: None,
            should_quit: false,
            config,
        }
    }

    /// Process an action and update application state.
    pub fn process_action(&mut self, action: Action) {
        self.message = None;

        match action {
            // Navigation
            Action::CursorUp => {
                let is_visual = self.input.mode == Mode::Visual;
                let pane = self.active_pane_mut();
                let was_cursor = pane.cursor;
                pane.cursor_up();
                if is_visual && was_cursor != pane.cursor {
                    pane.toggle_selection();
                }
            }
            Action::CursorDown => {
                let is_visual = self.input.mode == Mode::Visual;
                let pane = self.active_pane_mut();
                let was_cursor = pane.cursor;
                pane.cursor_down();
                if is_visual && was_cursor != pane.cursor {
                    pane.toggle_selection();
                }
            }
            Action::CursorTop => self.active_pane_mut().cursor_top(),
            Action::CursorBottom => self.active_pane_mut().cursor_bottom(),
            Action::EnterDir => {
                // Extract entry info first to avoid overlapping borrows
                let entry_info = self.active_pane().current_entry().map(|e| {
                    (e.path.clone(), e.is_dir)
                });
                if let Some((path, is_dir)) = entry_info {
                    if is_dir {
                        self.bookmarks.visit(path.clone());
                        self.active_pane_mut().navigate_to(path);
                        self.tabs.active_tab_mut().update_name();
                    } else {
                        self.open_file(&path);
                    }
                }
            }
            Action::ParentDir => {
                let pane = self.active_pane_mut();
                if pane.go_parent() {
                    let path = pane.path.clone();
                    self.bookmarks.visit(path);
                    let tab = self.tabs.active_tab_mut();
                    tab.update_name();
                }
            }
            Action::OpenFile => {
                if let Some(entry) = self.active_pane().current_entry() {
                    let path = entry.path.clone();
                    if entry.is_dir {
                        self.bookmarks.visit(path.clone());
                        self.active_pane_mut().navigate_to(path);
                        self.tabs.active_tab_mut().update_name();
                    } else {
                        self.open_file(&path);
                    }
                }
            }

            // Selection
            Action::ToggleSelect => {
                self.active_pane_mut().toggle_selection();
                self.active_pane_mut().cursor_down();
            }
            Action::SelectAll => self.active_pane_mut().select_all(),
            Action::ClearSelection => self.active_pane_mut().clear_selection(),

            // File operations
            Action::Yank => {
                let paths = self.active_pane().selected_paths();
                if paths.is_empty() {
                    self.message = Some("Nothing to yank".to_string());
                } else {
                    let count = paths.len();
                    self.clipboard = Some(Clipboard {
                        paths,
                        is_cut: false,
                    });
                    self.message = Some(format!("Yanked {count} item(s)"));
                }
            }
            Action::Cut => {
                let paths = self.active_pane().selected_paths();
                if paths.is_empty() {
                    self.message = Some("Nothing to cut".to_string());
                } else {
                    let count = paths.len();
                    self.clipboard = Some(Clipboard {
                        paths,
                        is_cut: true,
                    });
                    self.message = Some(format!("Cut {count} item(s)"));
                }
            }
            Action::Paste => self.paste_clipboard(),
            Action::Delete => self.delete_selected(),

            Action::RenameStart => {
                if let Some(entry) = self.active_pane().current_entry() {
                    self.input.input_buffer = entry.name.clone();
                }
            }
            Action::RenameConfirm(new_name) => {
                if let Some(entry) = self.active_pane().current_entry() {
                    let path = entry.path.clone();
                    match crate::fs::rename_entry(&path, &new_name) {
                        Ok(_) => {
                            self.message = Some(format!("Renamed to {new_name}"));
                            self.active_pane_mut().refresh();
                        }
                        Err(e) => {
                            self.message = Some(format!("Rename failed: {e}"));
                        }
                    }
                }
            }
            Action::RenameCancel => {
                self.message = Some("Rename cancelled".to_string());
            }

            Action::CreateStart { .. } => {
                // Mode already set by input handler
            }
            Action::CreateFile(name) => {
                let dir = self.active_pane().path.clone();
                let path = dir.join(&name);
                match crate::fs::create_file(&path) {
                    Ok(()) => {
                        self.message = Some(format!("Created file: {name}"));
                        self.active_pane_mut().refresh();
                    }
                    Err(e) => {
                        self.message = Some(format!("Create failed: {e}"));
                    }
                }
            }
            Action::CreateDir(name) => {
                let dir = self.active_pane().path.clone();
                let path = dir.join(&name);
                match crate::fs::create_directory(&path) {
                    Ok(()) => {
                        self.message = Some(format!("Created directory: {name}"));
                        self.active_pane_mut().refresh();
                    }
                    Err(e) => {
                        self.message = Some(format!("Create failed: {e}"));
                    }
                }
            }

            // View
            Action::ToggleHidden => {
                self.active_pane_mut().toggle_hidden();
            }
            Action::TogglePane => {
                self.tabs.active_tab_mut().panes.toggle_active();
            }
            Action::Refresh => {
                self.active_pane_mut().refresh();
                self.message = Some("Refreshed".to_string());
            }

            // Search
            Action::SearchStart => {}
            Action::SearchUpdate(query) => {
                self.active_pane_mut().set_filter(query);
            }
            Action::SearchConfirm => {
                // Keep the filter active
            }
            Action::SearchCancel => {
                self.active_pane_mut().set_filter(String::new());
            }
            Action::SearchNext => {
                // Move to next match (just cursor down for now)
                self.active_pane_mut().cursor_down();
            }
            Action::SearchPrev => {
                self.active_pane_mut().cursor_up();
            }

            // Tabs
            Action::NewTab => {
                let path = self.active_pane().path.clone();
                self.tabs.open_tab(path, &self.config);
            }
            Action::CloseTab => {
                if !self.tabs.close_tab() {
                    self.message = Some("Cannot close last tab".to_string());
                }
            }
            Action::NextTab => self.tabs.next_tab(),
            Action::PrevTab => self.tabs.prev_tab(),

            // Bookmarks
            Action::BookmarkAdd => {
                let path = self.active_pane().path.clone();
                if self.bookmarks.add_bookmark(path.clone()) {
                    self.message = Some(format!("Bookmarked: {}", path.display()));
                } else {
                    self.message = Some("Already bookmarked".to_string());
                }
            }
            Action::BookmarkGo(index) => {
                if let Some(path) = self.bookmarks.bookmarks.get(index).cloned() {
                    self.active_pane_mut().navigate_to(path);
                    self.tabs.active_tab_mut().update_name();
                }
            }

            // Sort
            Action::SortByName => {
                let pane = self.active_pane_mut();
                let new_dir = if pane.sort_field == SortField::Name
                    && pane.sort_dir == SortDirection::Ascending
                {
                    SortDirection::Descending
                } else {
                    SortDirection::Ascending
                };
                pane.set_sort(SortField::Name, new_dir);
            }
            Action::SortBySize => {
                let pane = self.active_pane_mut();
                let new_dir = if pane.sort_field == SortField::Size
                    && pane.sort_dir == SortDirection::Descending
                {
                    SortDirection::Ascending
                } else {
                    SortDirection::Descending
                };
                pane.set_sort(SortField::Size, new_dir);
            }
            Action::SortByModified => {
                let pane = self.active_pane_mut();
                pane.set_sort(SortField::Modified, SortDirection::Descending);
            }
            Action::SortByExtension => {
                let pane = self.active_pane_mut();
                pane.set_sort(SortField::Extension, SortDirection::Ascending);
            }

            // Command
            Action::CommandStart => {}
            Action::CommandExecute(cmd) => {
                let parsed = crate::input::parse_command(&cmd);
                // Handle cd specially
                if cmd.starts_with("cd ") {
                    let path_str = cmd.strip_prefix("cd ").unwrap_or("").trim();
                    let path = crate::bookmarks::expand_tilde(path_str);
                    if path.is_dir() {
                        self.active_pane_mut().navigate_to(path);
                        self.tabs.active_tab_mut().update_name();
                    } else {
                        self.message = Some(format!("Not a directory: {}", path.display()));
                    }
                } else {
                    self.process_action(parsed);
                }
            }
            Action::CommandCancel => {}

            // Mode switching
            Action::EnterVisual => {
                self.active_pane_mut().toggle_selection();
            }
            Action::ExitVisual => {
                self.active_pane_mut().clear_selection();
            }

            // App
            Action::Quit => self.should_quit = true,
            Action::None => {}
        }
    }

    /// Get the active pane (immutable).
    pub fn active_pane(&self) -> &crate::pane::Pane {
        self.tabs.active_tab().panes.active()
    }

    /// Get the active pane (mutable).
    pub fn active_pane_mut(&mut self) -> &mut crate::pane::Pane {
        self.tabs.active_tab_mut().panes.active_mut()
    }

    fn paste_clipboard(&mut self) {
        let clipboard = match self.clipboard.take() {
            Some(c) => c,
            None => {
                self.message = Some("Clipboard is empty".to_string());
                return;
            }
        };

        let dst_dir = self.active_pane().path.clone();
        let mut ok_count = 0;
        let mut err_count = 0;

        for src in &clipboard.paths {
            let name = src
                .file_name()
                .map_or_else(|| "unnamed".to_string(), |n| n.to_string_lossy().into_owned());
            let dst = dst_dir.join(&name);

            let result = if clipboard.is_cut {
                crate::fs::move_entry(src, &dst)
            } else {
                crate::fs::copy_entry(src, &dst)
            };

            match result {
                Ok(()) => ok_count += 1,
                Err(e) => {
                    tracing::warn!("paste failed for {}: {e}", src.display());
                    err_count += 1;
                }
            }
        }

        let op = if clipboard.is_cut { "Moved" } else { "Copied" };
        if err_count == 0 {
            self.message = Some(format!("{op} {ok_count} item(s)"));
        } else {
            self.message = Some(format!("{op} {ok_count}, failed {err_count}"));
        }

        // Put clipboard back if it was a copy (can paste multiple times)
        if !clipboard.is_cut {
            self.clipboard = Some(clipboard);
        }

        self.active_pane_mut().refresh();
    }

    fn delete_selected(&mut self) {
        let paths = self.active_pane().selected_paths();
        if paths.is_empty() {
            self.message = Some("Nothing selected to delete".to_string());
            return;
        }

        let mut ok_count = 0;
        let mut err_count = 0;

        for path in &paths {
            match crate::fs::trash_entry(path) {
                Ok(()) => ok_count += 1,
                Err(e) => {
                    tracing::warn!("trash failed for {}: {e}", path.display());
                    err_count += 1;
                }
            }
        }

        if err_count == 0 {
            self.message = Some(format!("Trashed {ok_count} item(s)"));
        } else {
            self.message = Some(format!("Trashed {ok_count}, failed {err_count}"));
        }

        self.active_pane_mut().refresh();
    }

    fn open_file(&mut self, path: &std::path::Path) {
        let ops = crate::platform::create_file_ops();
        match ops.open_file(path) {
            Ok(()) => {
                self.message = Some(format!(
                    "Opened: {}",
                    path.file_name()
                        .map_or_else(|| "?".to_string(), |n| n.to_string_lossy().into_owned())
                ));
            }
            Err(e) => {
                self.message = Some(format!("Open failed: {e}"));
            }
        }
    }
}
