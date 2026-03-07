//! Tanken configuration — uses shikumi for discovery and hot-reload.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct TankenConfig {
    pub appearance: AppearanceConfig,
    pub navigation: NavigationConfig,
    pub search: SearchConfig,
    pub preview: PreviewConfig,
    pub daemon: DaemonConfig,
}

impl Default for TankenConfig {
    fn default() -> Self {
        Self {
            appearance: AppearanceConfig::default(),
            navigation: NavigationConfig::default(),
            search: SearchConfig::default(),
            preview: PreviewConfig::default(),
            daemon: DaemonConfig::default(),
        }
    }
}

/// Visual appearance settings.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct AppearanceConfig {
    /// Window width in pixels.
    pub width: u32,
    /// Window height in pixels.
    pub height: u32,
    /// Font size in points.
    pub font_size: f32,
    /// Background opacity (0.0-1.0).
    pub opacity: f32,
    /// Show hidden files (dotfiles).
    pub show_hidden: bool,
    /// Icon size in pixels.
    pub icon_size: u32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            width: 900,
            height: 600,
            font_size: 14.0,
            opacity: 0.95,
            show_hidden: false,
            icon_size: 24,
        }
    }
}

/// Navigation settings.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct NavigationConfig {
    /// Default directory to open.
    pub default_path: PathBuf,
    /// Bookmarked directories.
    pub bookmarks: Vec<String>,
    /// Show sidebar with bookmarks.
    pub show_sidebar: bool,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self {
            default_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
            bookmarks: vec![
                "~/Documents".into(),
                "~/Downloads".into(),
                "~/Desktop".into(),
            ],
            show_sidebar: true,
        }
    }
}

/// Search / indexing configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct SearchConfig {
    /// Directories to index for fast search.
    pub index_dirs: Vec<PathBuf>,
    /// Glob patterns to exclude from indexing.
    pub exclude_patterns: Vec<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            index_dirs: vec![dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))],
            exclude_patterns: vec![
                "*.DS_Store".into(),
                "node_modules".into(),
                ".git".into(),
            ],
        }
    }
}

/// File preview settings.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct PreviewConfig {
    /// Enable file preview panel.
    pub enabled: bool,
    /// Maximum file size for preview (MB).
    pub max_file_size_mb: u32,
    /// Enable syntax highlighting in preview.
    pub syntax_highlighting: bool,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_file_size_mb: 10,
            syntax_highlighting: true,
        }
    }
}

/// Daemon mode configuration.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct DaemonConfig {
    /// Enable background file watching daemon.
    pub enable: bool,
    /// Directories to watch for changes.
    pub watch_dirs: Vec<PathBuf>,
    /// Interval in seconds between index refreshes.
    pub index_interval_secs: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enable: false,
            watch_dirs: vec![dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))],
            index_interval_secs: 300,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = TankenConfig::default();
        assert_eq!(config.appearance.width, 900);
        assert_eq!(config.appearance.height, 600);
        assert!(config.preview.enabled);
        assert!(!config.daemon.enable);
    }

    #[test]
    fn default_navigation_has_bookmarks() {
        let config = TankenConfig::default();
        assert!(!config.navigation.bookmarks.is_empty());
        assert!(config.navigation.show_sidebar);
    }
}
