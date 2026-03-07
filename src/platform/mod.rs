//! Platform abstraction traits.
//!
//! File operations, metadata, and trash management.
//! Platform-specific implementations live in submodules.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(target_os = "macos")]
pub mod macos;

/// A single entry in a directory listing.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// File or directory name.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Size in bytes.
    pub size: u64,
    /// Last modification time.
    pub modified: SystemTime,
}

/// Detailed file information.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File name.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// Permissions string (e.g. "rwxr-xr-x").
    pub permissions: String,
    /// Creation time.
    pub created: SystemTime,
    /// Last modification time.
    pub modified: SystemTime,
}

/// File system operations.
pub trait FileOperations: Send + Sync {
    /// List the contents of a directory.
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, Box<dyn std::error::Error>>;

    /// Open a file with the system default application.
    fn open_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>>;

    /// Move a file or directory to the trash.
    fn move_to_trash(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>>;

    /// Get detailed information about a file.
    fn get_file_info(&self, path: &Path) -> Result<FileInfo, Box<dyn std::error::Error>>;
}

/// Create a platform-specific file operations implementation.
pub fn create_file_ops() -> Box<dyn FileOperations> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSFileOperations)
    }
    #[cfg(not(target_os = "macos"))]
    {
        panic!("file operations not implemented for this platform")
    }
}
