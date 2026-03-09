//! File preview: text content, directory summaries, metadata.
//!
//! Dispatches preview generation based on file type. Text files get
//! content with line numbers, directories get child listings, and
//! binary files get metadata.

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::pane;

/// Maximum lines to read for text preview.
const MAX_PREVIEW_LINES: usize = 200;

/// Maximum file size to attempt text preview (10 MB).
const MAX_TEXT_SIZE: u64 = 10 * 1024 * 1024;

/// A preview of a file or directory.
#[derive(Debug, Clone)]
pub enum Preview {
    /// Text file content (lines).
    Text {
        lines: Vec<String>,
        language: String,
        total_lines: usize,
    },
    /// Directory listing summary.
    Directory {
        path: String,
        children: Vec<DirChild>,
        total_size: u64,
    },
    /// Binary file or unsupported — just show metadata.
    Metadata {
        file_type: String,
        size: u64,
        permissions: String,
        modified: String,
    },
    /// Image file metadata.
    Image {
        width: u32,
        height: u32,
        format: String,
        size: u64,
    },
    /// Nothing to preview (empty directory, error, etc.).
    Empty(String),
}

/// A child entry in a directory preview.
#[derive(Debug, Clone)]
pub struct DirChild {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Generate a preview for the given path.
#[must_use]
pub fn generate_preview(path: &Path) -> Preview {
    if !path.exists() {
        return Preview::Empty("File not found".to_string());
    }

    if path.is_dir() {
        return preview_directory(path);
    }

    let metadata = match path.metadata() {
        Ok(m) => m,
        Err(e) => return Preview::Empty(format!("Cannot read: {e}")),
    };

    let size = metadata.len();

    // Check for image formats
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        if matches!(ext_lower.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tiff") {
            return preview_image(path, &ext_lower, size);
        }
    }

    // Try text preview
    if size <= MAX_TEXT_SIZE && is_likely_text(path) {
        return preview_text(path);
    }

    // Fallback to metadata
    preview_metadata(path, size)
}

fn preview_directory(path: &Path) -> Preview {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => return Preview::Empty(format!("Cannot read directory: {e}")),
    };

    let mut children = Vec::new();
    let mut total_size = 0u64;

    for entry in entries.flatten() {
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
        let size = meta.as_ref().map_or(0, |m| m.len());
        total_size += size;

        children.push(DirChild {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir,
            size,
        });
    }

    children.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    Preview::Directory {
        path: path.display().to_string(),
        children,
        total_size,
    }
}

fn preview_text(path: &Path) -> Preview {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return Preview::Empty(format!("Cannot open: {e}")),
    };

    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut total_lines = 0;

    for line in reader.lines() {
        total_lines += 1;
        if lines.len() < MAX_PREVIEW_LINES {
            match line {
                Ok(l) => lines.push(l),
                Err(_) => {
                    // Hit non-UTF-8 data, stop
                    break;
                }
            }
        }
    }

    let language = detect_language(path);

    Preview::Text {
        lines,
        language,
        total_lines,
    }
}

fn preview_image(path: &Path, format: &str, size: u64) -> Preview {
    // We cannot decode the actual image dimensions without an image crate.
    // Return metadata-only preview.
    let _ = path;
    Preview::Image {
        width: 0,
        height: 0,
        format: format.to_uppercase(),
        size,
    }
}

fn preview_metadata(path: &Path, size: u64) -> Preview {
    let permissions = pane::format_permissions(path);
    let modified = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map_or_else(|| "unknown".to_string(), pane::format_time);

    let file_type = path
        .extension()
        .and_then(|e| e.to_str())
        .map_or_else(|| "binary".to_string(), |e| e.to_uppercase());

    Preview::Metadata {
        file_type,
        size,
        permissions,
        modified,
    }
}

/// Detect the programming language from file extension.
#[must_use]
fn detect_language(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map_or_else(|| "text".to_string(), |ext| {
            match ext.to_lowercase().as_str() {
                "rs" => "rust",
                "py" => "python",
                "js" | "mjs" | "cjs" => "javascript",
                "ts" | "mts" | "cts" => "typescript",
                "tsx" | "jsx" => "react",
                "go" => "go",
                "c" | "h" => "c",
                "cpp" | "cc" | "cxx" | "hpp" => "cpp",
                "java" => "java",
                "rb" => "ruby",
                "sh" | "bash" | "zsh" => "shell",
                "nix" => "nix",
                "toml" => "toml",
                "yaml" | "yml" => "yaml",
                "json" => "json",
                "xml" | "html" | "htm" => "xml",
                "css" | "scss" | "sass" => "css",
                "md" | "markdown" => "markdown",
                "lua" => "lua",
                "sql" => "sql",
                "dockerfile" => "dockerfile",
                _ => ext,
            }
            .to_string()
        })
}

/// Heuristic check: is the file likely text (not binary)?
fn is_likely_text(path: &Path) -> bool {
    // Check extension first
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        let text_exts = [
            "txt", "md", "rs", "py", "js", "ts", "tsx", "jsx", "go", "c", "h",
            "cpp", "cc", "hpp", "java", "rb", "sh", "bash", "zsh", "nix", "toml",
            "yaml", "yml", "json", "xml", "html", "htm", "css", "scss", "sass",
            "lua", "sql", "dockerfile", "makefile", "cmake", "conf", "cfg", "ini",
            "env", "log", "csv", "lock", "gitignore", "gitattributes", "editorconfig",
            "flake", "service", "timer", "socket", "mount",
        ];
        if text_exts.contains(&ext_lower.as_str()) {
            return true;
        }
    }

    // Check by reading first few bytes for null bytes
    if let Ok(file) = fs::File::open(path) {
        let reader = BufReader::new(file);
        let mut buf = Vec::new();
        let bytes_read = reader
            .take(8192)
            .read_to_end(&mut buf)
            .unwrap_or(0);

        if bytes_read == 0 {
            return true; // Empty files are "text"
        }

        // If we find a null byte in the first 8KB, it's probably binary
        return !buf.contains(&0);
    }

    false
}

/// Format a preview into lines of text for rendering.
#[must_use]
pub fn preview_to_lines(preview: &Preview) -> Vec<String> {
    match preview {
        Preview::Text { lines, language, total_lines } => {
            let mut out = vec![format!(" [{language}] {total_lines} lines")];
            out.push(String::new());
            for (i, line) in lines.iter().enumerate() {
                out.push(format!(" {:>4} | {line}", i + 1));
            }
            if lines.len() < *total_lines {
                out.push(format!("  ... ({} more lines)", total_lines - lines.len()));
            }
            out
        }
        Preview::Directory { path: _, children, total_size } => {
            let size_str = pane::format_size(*total_size);
            let dirs = children.iter().filter(|c| c.is_dir).count();
            let files = children.len() - dirs;
            let mut out = vec![
                format!(" {dirs} dirs, {files} files ({size_str})"),
                String::new(),
            ];
            for child in children.iter().take(50) {
                let icon = if child.is_dir { "/" } else { "" };
                out.push(format!("  {}{icon}", child.name));
            }
            if children.len() > 50 {
                out.push(format!("  ... ({} more)", children.len() - 50));
            }
            out
        }
        Preview::Metadata { file_type, size, permissions, modified } => {
            let size_str = pane::format_size(*size);
            vec![
                format!(" Type: {file_type}"),
                format!(" Size: {size_str}"),
                format!(" Perm: {permissions}"),
                format!(" Modified: {modified}"),
            ]
        }
        Preview::Image { format, size, .. } => {
            let size_str = pane::format_size(*size);
            vec![
                format!(" Image: {format}"),
                format!(" Size: {size_str}"),
                String::new(),
                " (image preview requires GPU renderer)".to_string(),
            ]
        }
        Preview::Empty(msg) => {
            vec![format!(" {msg}")]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_nonexistent() {
        let preview = generate_preview(Path::new("/nonexistent_12345"));
        matches!(preview, Preview::Empty(_));
    }

    #[test]
    fn preview_text_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("test.rs");
        fs::write(&file, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        let preview = generate_preview(&file);
        match preview {
            Preview::Text { lines, language, total_lines } => {
                assert_eq!(language, "rust");
                assert_eq!(total_lines, 3);
                assert!(lines[0].contains("fn main"));
            }
            _ => panic!("expected text preview"),
        }
    }

    #[test]
    fn preview_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap();
        fs::create_dir(tmp.path().join("subdir")).unwrap();
        let preview = generate_preview(tmp.path());
        match preview {
            Preview::Directory { children, .. } => {
                assert_eq!(children.len(), 2);
                // Dirs should come first
                assert!(children[0].is_dir);
            }
            _ => panic!("expected directory preview"),
        }
    }

    #[test]
    fn detect_language_works() {
        assert_eq!(detect_language(Path::new("foo.rs")), "rust");
        assert_eq!(detect_language(Path::new("foo.py")), "python");
        assert_eq!(detect_language(Path::new("foo.nix")), "nix");
        assert_eq!(detect_language(Path::new("foo")), "text");
    }
}
