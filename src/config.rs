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

    // ── Appearance defaults ──────────────────────────────────────────

    #[test]
    fn default_appearance_pins_every_field() {
        // These are the values the CLAUDE.md schema documents. A
        // refactor that silently changes any of them would ship
        // users a window of the wrong size / opacity / font.
        let a = AppearanceConfig::default();
        assert_eq!(a.width, 900);
        assert_eq!(a.height, 600);
        assert!((a.font_size - 14.0).abs() < f32::EPSILON);
        assert!((a.opacity - 0.95).abs() < f32::EPSILON);
        assert!(!a.show_hidden);
        assert_eq!(a.icon_size, 24);
    }

    // ── Navigation defaults ──────────────────────────────────────────

    #[test]
    fn default_navigation_bookmark_set() {
        // Documented default set: ~/Documents, ~/Downloads, ~/Desktop.
        // Order matters for sidebar rendering — pin positional.
        let n = NavigationConfig::default();
        assert_eq!(n.bookmarks.len(), 3);
        assert_eq!(n.bookmarks[0], "~/Documents");
        assert_eq!(n.bookmarks[1], "~/Downloads");
        assert_eq!(n.bookmarks[2], "~/Desktop");
        assert!(n.show_sidebar);
    }

    #[test]
    fn default_navigation_path_is_absolute() {
        // `dirs::home_dir().unwrap_or(PathBuf::from("/"))` — the
        // fallback "/" must still be absolute, and the derived
        // path under any user must be absolute too. Catches a
        // refactor that uses a relative "." fallback.
        let n = NavigationConfig::default();
        assert!(
            n.default_path.is_absolute(),
            "default_path not absolute: {:?}",
            n.default_path
        );
    }

    // ── Search defaults ──────────────────────────────────────────────

    #[test]
    fn default_search_excludes_three_noise_sources() {
        // The documented noise sources: macOS metadata, JS deps, git.
        // Losing any one would balloon the tantivy index size.
        let s = SearchConfig::default();
        assert_eq!(s.exclude_patterns.len(), 3);
        assert!(s.exclude_patterns.iter().any(|p| p == "*.DS_Store"));
        assert!(s.exclude_patterns.iter().any(|p| p == "node_modules"));
        assert!(s.exclude_patterns.iter().any(|p| p == ".git"));
    }

    #[test]
    fn default_search_indexes_home_dir() {
        let s = SearchConfig::default();
        assert_eq!(s.index_dirs.len(), 1);
        assert!(s.index_dirs[0].is_absolute());
    }

    // ── Preview defaults ─────────────────────────────────────────────

    #[test]
    fn default_preview_pins_every_field() {
        let p = PreviewConfig::default();
        assert!(p.enabled);
        assert_eq!(p.max_file_size_mb, 10);
        assert!(p.syntax_highlighting);
    }

    // ── Daemon defaults ──────────────────────────────────────────────

    #[test]
    fn default_daemon_is_off_with_five_minute_refresh() {
        // Daemon is opt-in (default off); the 300s (5 min) interval
        // is documented. Pin the refresh interval — a shorter default
        // would pound the filesystem, a longer one would mean stale
        // search results.
        let d = DaemonConfig::default();
        assert!(!d.enable);
        assert_eq!(d.index_interval_secs, 300);
        assert_eq!(d.watch_dirs.len(), 1);
    }

    // ── Serde: top-level default behaviour ───────────────────────────

    #[test]
    fn empty_json_object_produces_full_defaults() {
        // `#[serde(default)]` at the top means an empty TankenConfig
        // YAML/JSON must fill in every sub-default, not error.
        // This is load-bearing — a user's fresh tanken.yaml contains
        // only the keys they override.
        let c: TankenConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(c.appearance.width, 900);
        assert_eq!(c.navigation.bookmarks.len(), 3);
        assert!(!c.daemon.enable);
        assert!(c.preview.enabled);
    }

    #[test]
    fn partial_json_fills_in_untouched_defaults() {
        // Override just appearance.width; everything else stays at
        // default. This is the canonical hot-reload scenario — a user
        // edits one field and the rest must survive unchanged.
        let c: TankenConfig = serde_json::from_str(
            r#"{"appearance": {"width": 1200}}"#,
        )
        .unwrap();
        assert_eq!(c.appearance.width, 1200);
        assert_eq!(c.appearance.height, 600); // untouched
        assert_eq!(c.preview.max_file_size_mb, 10); // untouched
    }

    #[test]
    fn round_trip_through_json_preserves_every_field() {
        let original = TankenConfig::default();
        let json = serde_json::to_string(&original).unwrap();
        let back: TankenConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.appearance.width, original.appearance.width);
        assert_eq!(back.navigation.bookmarks, original.navigation.bookmarks);
        assert_eq!(back.search.exclude_patterns, original.search.exclude_patterns);
        assert_eq!(back.daemon.index_interval_secs, original.daemon.index_interval_secs);
    }

    #[test]
    fn custom_daemon_enable_roundtrips() {
        // User overrides daemon.enable=true; roundtrip must preserve.
        let c: TankenConfig = serde_json::from_str(
            r#"{"daemon": {"enable": true, "index_interval_secs": 60}}"#,
        )
        .unwrap();
        assert!(c.daemon.enable);
        assert_eq!(c.daemon.index_interval_secs, 60);
        // Default watch_dirs still populated via #[serde(default)].
        assert_eq!(c.daemon.watch_dirs.len(), 1);
    }

    #[test]
    fn unknown_top_level_key_is_ignored() {
        // serde default behaviour: unknown keys are ignored (no
        // deny_unknown_fields). A user with an obsolete key in their
        // config shouldn't see a parse failure.
        let c: TankenConfig = serde_json::from_str(
            r#"{"some_removed_section": {"foo": "bar"}}"#,
        )
        .unwrap();
        // Still produces defaults.
        assert_eq!(c.appearance.width, 900);
    }
}
