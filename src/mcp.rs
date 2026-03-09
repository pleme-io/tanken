//! MCP server for tanken file manager.
//!
//! Provides tools for listing directories, previewing files, copying,
//! moving, creating directories, and searching the filesystem.

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;

// ── Tool input types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListDirInput {
    #[schemars(description = "Directory path to list. Defaults to current directory.")]
    path: Option<String>,
    #[schemars(description = "Include hidden files (default: false).")]
    show_hidden: Option<bool>,
    #[schemars(description = "Sort by: 'name', 'size', 'modified', or 'type'.")]
    sort: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetPreviewInput {
    #[schemars(description = "Path to the file or directory to preview.")]
    path: String,
    #[schemars(description = "Maximum number of lines for text preview (default: 50).")]
    max_lines: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CopyFileInput {
    #[schemars(description = "Source file or directory path.")]
    src: String,
    #[schemars(description = "Destination path.")]
    dst: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MoveFileInput {
    #[schemars(description = "Source file or directory path.")]
    src: String,
    #[schemars(description = "Destination path.")]
    dst: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateDirInput {
    #[schemars(description = "Path for the new directory. Parent directories are created as needed.")]
    path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SearchInput {
    #[schemars(description = "Search query (filename pattern or text to find).")]
    query: String,
    #[schemars(description = "Directory to search in. Defaults to current directory.")]
    path: Option<String>,
    #[schemars(description = "Search file contents, not just names (default: false).")]
    content: Option<bool>,
    #[schemars(description = "Maximum number of results (default: 50).")]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfigGetInput {
    #[schemars(description = "Config key to retrieve. Omit for full config.")]
    key: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfigSetInput {
    #[schemars(description = "Config key to set.")]
    key: String,
    #[schemars(description = "Value to set (as JSON string).")]
    value: String,
}

// ── MCP Server ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct TankenMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl TankenMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    // ── Standard tools ──────────────────────────────────────────────────────

    #[tool(description = "Get tanken application status and health information.")]
    async fn status(&self) -> String {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        serde_json::json!({
            "status": "running",
            "app": "tanken",
            "cwd": cwd,
        })
        .to_string()
    }

    #[tool(description = "Get tanken version information.")]
    async fn version(&self) -> String {
        serde_json::json!({
            "name": "tanken",
            "version": env!("CARGO_PKG_VERSION"),
            "description": env!("CARGO_PKG_DESCRIPTION"),
        })
        .to_string()
    }

    #[tool(description = "Get a tanken configuration value. Pass a key for a specific value, or omit for the full config.")]
    async fn config_get(&self, Parameters(input): Parameters<ConfigGetInput>) -> String {
        match input.key {
            Some(key) => serde_json::json!({
                "key": key,
                "value": null,
                "note": "Config queries require a running tanken instance."
            })
            .to_string(),
            None => serde_json::json!({
                "config_path": "~/.config/tanken/tanken.yaml"
            })
            .to_string(),
        }
    }

    #[tool(description = "Set a tanken configuration value at runtime.")]
    async fn config_set(&self, Parameters(input): Parameters<ConfigSetInput>) -> String {
        serde_json::json!({
            "key": input.key,
            "value": input.value,
            "applied": false,
            "note": "Config mutations require a running tanken instance."
        })
        .to_string()
    }

    // ── File manager tools ──────────────────────────────────────────────────

    #[tool(description = "List contents of a directory. Returns file names, types, sizes, and modification dates.")]
    async fn list_dir(&self, Parameters(input): Parameters<ListDirInput>) -> String {
        let dir = input.path.unwrap_or_else(|| ".".to_string());
        let path = std::path::Path::new(&dir);
        let show_hidden = input.show_hidden.unwrap_or(false);

        if !path.is_dir() {
            return serde_json::json!({"error": format!("not a directory: {dir}")}).to_string();
        }

        let mut entries = Vec::new();
        if let Ok(read_dir) = std::fs::read_dir(path) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                let meta = entry.metadata().ok();
                let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                entries.push(serde_json::json!({
                    "name": name,
                    "is_dir": is_dir,
                    "size_bytes": if is_dir { None } else { meta.as_ref().map(|m| m.len()) },
                }));
            }
        }

        // Sort
        let sort = input.sort.unwrap_or_else(|| "name".to_string());
        match sort.as_str() {
            "size" => entries.sort_by(|a, b| {
                let sa = a["size_bytes"].as_u64().unwrap_or(0);
                let sb = b["size_bytes"].as_u64().unwrap_or(0);
                sb.cmp(&sa)
            }),
            "name" | _ => entries.sort_by(|a, b| {
                let da = a["is_dir"].as_bool().unwrap_or(false);
                let db = b["is_dir"].as_bool().unwrap_or(false);
                db.cmp(&da).then_with(|| {
                    let na = a["name"].as_str().unwrap_or("");
                    let nb = b["name"].as_str().unwrap_or("");
                    na.to_lowercase().cmp(&nb.to_lowercase())
                })
            }),
        }

        serde_json::json!({
            "directory": dir,
            "count": entries.len(),
            "entries": entries,
        })
        .to_string()
    }

    #[tool(description = "Get a preview of a file or directory. For text files, returns the first N lines. For directories, returns a content summary.")]
    async fn get_preview(&self, Parameters(input): Parameters<GetPreviewInput>) -> String {
        let path = std::path::Path::new(&input.path);
        let max_lines = input.max_lines.unwrap_or(50);

        if !path.exists() {
            return serde_json::json!({"error": format!("not found: {}", input.path)}).to_string();
        }

        if path.is_dir() {
            let count = std::fs::read_dir(path)
                .map(|rd| rd.count())
                .unwrap_or(0);
            return serde_json::json!({
                "type": "directory",
                "path": input.path,
                "item_count": count,
            })
            .to_string();
        }

        // Try to read as text
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().take(max_lines).collect();
                let truncated = content.lines().count() > max_lines;
                serde_json::json!({
                    "type": "text",
                    "path": input.path,
                    "lines": lines,
                    "truncated": truncated,
                    "total_lines": content.lines().count(),
                })
                .to_string()
            }
            Err(_) => {
                let meta = std::fs::metadata(path).ok();
                serde_json::json!({
                    "type": "binary",
                    "path": input.path,
                    "size_bytes": meta.map(|m| m.len()),
                })
                .to_string()
            }
        }
    }

    #[tool(description = "Copy a file or directory to a new location.")]
    async fn copy_file(&self, Parameters(input): Parameters<CopyFileInput>) -> String {
        let src = std::path::Path::new(&input.src);
        let dst = std::path::Path::new(&input.dst);

        if !src.exists() {
            return serde_json::json!({"error": format!("source not found: {}", input.src)})
                .to_string();
        }

        if src.is_dir() {
            // Recursive directory copy
            match copy_dir_recursive(src, dst) {
                Ok(()) => serde_json::json!({"ok": true, "src": input.src, "dst": input.dst})
                    .to_string(),
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            }
        } else {
            match std::fs::copy(src, dst) {
                Ok(bytes) => {
                    serde_json::json!({"ok": true, "src": input.src, "dst": input.dst, "bytes_copied": bytes})
                        .to_string()
                }
                Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
            }
        }
    }

    #[tool(description = "Move a file or directory to a new location.")]
    async fn move_file(&self, Parameters(input): Parameters<MoveFileInput>) -> String {
        let src = std::path::Path::new(&input.src);

        if !src.exists() {
            return serde_json::json!({"error": format!("source not found: {}", input.src)})
                .to_string();
        }

        match std::fs::rename(&input.src, &input.dst) {
            Ok(()) => {
                serde_json::json!({"ok": true, "src": input.src, "dst": input.dst}).to_string()
            }
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(description = "Create a new directory. Parent directories are created as needed.")]
    async fn create_dir(&self, Parameters(input): Parameters<CreateDirInput>) -> String {
        match std::fs::create_dir_all(&input.path) {
            Ok(()) => serde_json::json!({"ok": true, "path": input.path}).to_string(),
            Err(e) => serde_json::json!({"error": e.to_string()}).to_string(),
        }
    }

    #[tool(description = "Search for files by name pattern in a directory tree. Optionally search file contents.")]
    async fn search(&self, Parameters(input): Parameters<SearchInput>) -> String {
        let dir = input.path.unwrap_or_else(|| ".".to_string());
        let path = std::path::Path::new(&dir);
        let limit = input.limit.unwrap_or(50);
        let search_content = input.content.unwrap_or(false);

        if !path.is_dir() {
            return serde_json::json!({"error": format!("not a directory: {dir}")}).to_string();
        }

        let query_lower = input.query.to_lowercase();
        let mut results = Vec::new();
        search_recursive(path, &query_lower, search_content, limit, &mut results);

        serde_json::json!({
            "query": input.query,
            "directory": dir,
            "content_search": search_content,
            "count": results.len(),
            "results": results,
        })
        .to_string()
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

fn search_recursive(
    dir: &std::path::Path,
    query: &str,
    search_content: bool,
    limit: usize,
    results: &mut Vec<serde_json::Value>,
) {
    if results.len() >= limit {
        return;
    }

    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        if results.len() >= limit {
            return;
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        // Skip hidden files and common ignore dirs
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }

        // Check filename match
        if name.to_lowercase().contains(query) {
            results.push(serde_json::json!({
                "path": path.display().to_string(),
                "name": name,
                "match_type": "filename",
            }));
        }

        // Check content match for text files
        if search_content && path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if content.to_lowercase().contains(query) {
                    // Find line number of first match
                    let line_num = content
                        .lines()
                        .position(|l| l.to_lowercase().contains(query))
                        .map(|n| n + 1);
                    results.push(serde_json::json!({
                        "path": path.display().to_string(),
                        "name": name,
                        "match_type": "content",
                        "line": line_num,
                    }));
                }
            }
        }

        // Recurse into directories
        if path.is_dir() {
            search_recursive(&path, query, search_content, limit, results);
        }
    }
}

#[tool_handler]
impl ServerHandler for TankenMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Tanken GPU file manager — directory listing, file preview, copy/move operations, and search."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let server = TankenMcp::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
