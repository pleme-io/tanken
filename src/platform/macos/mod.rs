//! macOS file operations implementation.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::platform::{FileEntry, FileInfo, FileOperations};

/// macOS-specific file operations.
pub struct MacOSFileOperations;

impl FileOperations for MacOSFileOperations {
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, Box<dyn std::error::Error>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
                modified: metadata.modified()?,
            });
        }
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(entries)
    }

    fn open_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        Command::new("open").arg(path).status()?;
        Ok(())
    }

    fn move_to_trash(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Use Finder's "move to trash" via osascript for proper trash behavior
        let script = format!(
            "tell application \"Finder\" to delete POSIX file \"{}\"",
            path.display()
        );
        Command::new("osascript").args(["-e", &script]).status()?;
        Ok(())
    }

    fn get_file_info(&self, path: &Path) -> Result<FileInfo, Box<dyn std::error::Error>> {
        let metadata = fs::metadata(path)?;
        let mode = metadata.permissions().mode();
        let permissions = format!("{mode:o}");
        Ok(FileInfo {
            name: path
                .file_name()
                .map_or_else(|| String::from("?"), |n| n.to_string_lossy().into_owned()),
            path: path.to_path_buf(),
            size: metadata.len(),
            permissions,
            created: metadata.created()?,
            modified: metadata.modified()?,
        })
    }
}
