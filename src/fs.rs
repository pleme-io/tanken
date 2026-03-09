//! Filesystem operations: copy, move, delete, rename, create.
//!
//! All operations work on the real filesystem using `std::fs` and provide
//! progress-aware async variants where applicable. Trash support is
//! delegated to the platform layer.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::platform::{self, FileEntry};

/// Errors from filesystem operations.
#[derive(Debug)]
pub enum FsError {
    Io(io::Error),
    NotFound(PathBuf),
    AlreadyExists(PathBuf),
    Platform(String),
}

impl std::fmt::Display for FsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::NotFound(p) => write!(f, "not found: {}", p.display()),
            Self::AlreadyExists(p) => write!(f, "already exists: {}", p.display()),
            Self::Platform(msg) => write!(f, "platform: {msg}"),
        }
    }
}

impl std::error::Error for FsError {}

impl From<io::Error> for FsError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// List directory contents, optionally including hidden files.
pub fn list_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, FsError> {
    if !path.exists() {
        return Err(FsError::NotFound(path.to_path_buf()));
    }

    let ops = platform::create_file_ops();
    let mut entries = ops
        .list_dir(path)
        .map_err(|e| FsError::Platform(e.to_string()))?;

    if !show_hidden {
        entries.retain(|e| !e.name.starts_with('.'));
    }

    Ok(entries)
}

/// Copy a file or directory recursively.
pub fn copy_entry(src: &Path, dst: &Path) -> Result<(), FsError> {
    if !src.exists() {
        return Err(FsError::NotFound(src.to_path_buf()));
    }
    if dst.exists() {
        return Err(FsError::AlreadyExists(dst.to_path_buf()));
    }

    if src.is_dir() {
        copy_dir_recursive(src, dst)?;
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), FsError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Move a file or directory.
pub fn move_entry(src: &Path, dst: &Path) -> Result<(), FsError> {
    if !src.exists() {
        return Err(FsError::NotFound(src.to_path_buf()));
    }
    if dst.exists() {
        return Err(FsError::AlreadyExists(dst.to_path_buf()));
    }

    // Try rename first (same filesystem), fallback to copy+delete
    if fs::rename(src, dst).is_err() {
        copy_entry(src, dst)?;
        if src.is_dir() {
            fs::remove_dir_all(src)?;
        } else {
            fs::remove_file(src)?;
        }
    }
    Ok(())
}

/// Delete a file or directory permanently.
pub fn delete_entry(path: &Path) -> Result<(), FsError> {
    if !path.exists() {
        return Err(FsError::NotFound(path.to_path_buf()));
    }

    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Move a file or directory to the system trash.
pub fn trash_entry(path: &Path) -> Result<(), FsError> {
    if !path.exists() {
        return Err(FsError::NotFound(path.to_path_buf()));
    }

    let ops = platform::create_file_ops();
    ops.move_to_trash(path)
        .map_err(|e| FsError::Platform(e.to_string()))
}

/// Rename a file or directory.
pub fn rename_entry(path: &Path, new_name: &str) -> Result<PathBuf, FsError> {
    if !path.exists() {
        return Err(FsError::NotFound(path.to_path_buf()));
    }

    let new_path = path
        .parent()
        .unwrap_or(Path::new("/"))
        .join(new_name);

    if new_path.exists() {
        return Err(FsError::AlreadyExists(new_path));
    }

    fs::rename(path, &new_path)?;
    Ok(new_path)
}

/// Create a new directory.
pub fn create_directory(path: &Path) -> Result<(), FsError> {
    if path.exists() {
        return Err(FsError::AlreadyExists(path.to_path_buf()));
    }
    fs::create_dir_all(path)?;
    Ok(())
}

/// Create a new empty file.
pub fn create_file(path: &Path) -> Result<(), FsError> {
    if path.exists() {
        return Err(FsError::AlreadyExists(path.to_path_buf()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::File::create(path)?;
    Ok(())
}

/// Get the total size of a directory recursively.
pub fn dir_size(path: &Path) -> u64 {
    if !path.is_dir() {
        return path.metadata().map_or(0, |m| m.len());
    }

    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else {
                total += p.metadata().map_or(0, |m| m.len());
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_and_list_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("test_dir");
        create_directory(&dir).unwrap();
        assert!(dir.exists());
        assert!(dir.is_dir());
    }

    #[test]
    fn create_and_delete_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        create_file(&file).unwrap();
        assert!(file.exists());
        delete_entry(&file).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn rename_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("old.txt");
        create_file(&file).unwrap();
        let new_path = rename_entry(&file, "new.txt").unwrap();
        assert!(!file.exists());
        assert!(new_path.exists());
        assert_eq!(new_path.file_name().unwrap().to_str().unwrap(), "new.txt");
    }

    #[test]
    fn copy_file() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src.txt");
        let dst = tmp.path().join("dst.txt");
        fs::write(&src, "hello").unwrap();
        copy_entry(&src, &dst).unwrap();
        assert!(src.exists());
        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "hello");
    }

    #[test]
    fn move_file() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src.txt");
        let dst = tmp.path().join("dst.txt");
        fs::write(&src, "hello").unwrap();
        move_entry(&src, &dst).unwrap();
        assert!(!src.exists());
        assert!(dst.exists());
    }

    #[test]
    fn copy_directory_recursive() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src_dir");
        let dst = tmp.path().join("dst_dir");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.txt"), "a").unwrap();
        fs::write(src.join("sub/b.txt"), "b").unwrap();
        copy_entry(&src, &dst).unwrap();
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("sub/b.txt").exists());
    }

    #[test]
    fn already_exists_error() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("existing.txt");
        create_file(&file).unwrap();
        let result = create_file(&file);
        assert!(result.is_err());
    }

    #[test]
    fn not_found_error() {
        let result = delete_entry(Path::new("/nonexistent_path_12345"));
        assert!(result.is_err());
    }
}
