//! Fuzzy file search using tankyu (sakuin) for indexed search
//! and fuzzy-matcher for live filtering.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// A search result with relevance score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Path to the matched file or directory.
    pub path: PathBuf,
    /// Display name.
    pub name: String,
    /// Whether it's a directory.
    pub is_dir: bool,
    /// Relevance score (higher = better match).
    pub score: i64,
}

/// Search engine with fuzzy matching and optional index support.
pub struct SearchEngine {
    matcher: SkimMatcherV2,
    index: Option<Arc<sakuin::IndexStore>>,
}

impl SearchEngine {
    /// Create a new search engine without index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
            index: None,
        }
    }

    /// Create a search engine with a tankyu index for fast lookups.
    pub fn with_index(index_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let spec = sakuin::SchemaSpec::new()
            .field("name", sakuin::TEXT | sakuin::STORED)
            .field("path", sakuin::STRING | sakuin::STORED)
            .field("is_dir", sakuin::STRING | sakuin::STORED);

        let store = sakuin::IndexStore::open(index_dir, &spec)?;

        Ok(Self {
            matcher: SkimMatcherV2::default(),
            index: Some(Arc::new(store)),
        })
    }

    /// Fuzzy search through a list of file entries.
    #[must_use]
    pub fn fuzzy_search(
        &self,
        query: &str,
        entries: &[crate::platform::FileEntry],
    ) -> Vec<SearchResult> {
        if query.is_empty() {
            return entries
                .iter()
                .map(|e| SearchResult {
                    path: e.path.clone(),
                    name: e.name.clone(),
                    is_dir: e.is_dir,
                    score: 0,
                })
                .collect();
        }

        let mut results: Vec<SearchResult> = entries
            .iter()
            .filter_map(|entry| {
                self.matcher.fuzzy_match(&entry.name, query).map(|score| {
                    SearchResult {
                        path: entry.path.clone(),
                        name: entry.name.clone(),
                        is_dir: entry.is_dir,
                        score,
                    }
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Search using the index (if available), falling back to live walk.
    pub fn search_indexed(
        &self,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        if let Some(ref index) = self.index {
            if let Ok(results) = index.search(query, &["name"], limit) {
                return results
                    .into_iter()
                    .filter_map(|(score, doc)| {
                        let name = doc.get("name")?.as_text()?.to_string();
                        let path = doc.get("path")?.as_text()?.to_string();
                        let is_dir = doc
                            .get("is_dir")
                            .and_then(|v| v.as_text())
                            .is_some_and(|v| v == "true");

                        Some(SearchResult {
                            path: PathBuf::from(path),
                            name,
                            is_dir,
                            #[allow(clippy::cast_possible_truncation)]
                            score: (score * 1000.0) as i64,
                        })
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    /// Index a directory's contents into the search index.
    pub fn index_directory(&self, path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
        let Some(ref index) = self.index else {
            return Ok(0);
        };

        let mut count = 0;
        index.write(|writer| {
            index_dir_recursive(writer, path, &mut count)?;
            Ok(())
        })?;

        tracing::info!("indexed {count} files from {}", path.display());
        Ok(count)
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn index_dir_recursive(
    writer: &mut sakuin::IndexWriter<'_>,
    path: &Path,
    count: &mut usize,
) -> Result<(), sakuin::TankyuError> {
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        // Skip hidden and common excluded directories
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "__pycache__"
        {
            continue;
        }

        let is_dir = entry_path.is_dir();
        let path_str = entry_path.to_string_lossy().into_owned();

        writer.add_doc(&[
            ("name", &name),
            ("path", &path_str),
            ("is_dir", if is_dir { "true" } else { "false" }),
        ])?;

        *count += 1;

        if is_dir {
            index_dir_recursive(writer, &entry_path, count)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::FileEntry;
    use std::time::SystemTime;

    fn make_entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            path: PathBuf::from(format!("/test/{name}")),
            is_dir,
            size: 0,
            modified: SystemTime::now(),
        }
    }

    #[test]
    fn fuzzy_search_basic() {
        let engine = SearchEngine::new();
        let entries = vec![
            make_entry("Cargo.toml", false),
            make_entry("Cargo.lock", false),
            make_entry("README.md", false),
            make_entry("src", true),
        ];

        let results = engine.fuzzy_search("cargo", &entries);
        assert_eq!(results.len(), 2);
        // Both Cargo files should match
        assert!(results.iter().all(|r| r.name.starts_with("Cargo")));
    }

    #[test]
    fn fuzzy_search_empty_query() {
        let engine = SearchEngine::new();
        let entries = vec![
            make_entry("a.txt", false),
            make_entry("b.txt", false),
        ];

        let results = engine.fuzzy_search("", &entries);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn fuzzy_search_no_match() {
        let engine = SearchEngine::new();
        let entries = vec![make_entry("hello.txt", false)];

        let results = engine.fuzzy_search("zzzzz", &entries);
        assert!(results.is_empty());
    }

    #[test]
    fn search_with_index() {
        let tmp = tempfile::TempDir::new().unwrap();
        let idx_dir = tmp.path().join("index");

        // Create some files to index
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::write(data_dir.join("hello.txt"), "hello world").unwrap();
        std::fs::write(data_dir.join("goodbye.rs"), "fn goodbye() {}").unwrap();

        let engine = SearchEngine::with_index(&idx_dir).unwrap();
        let count = engine.index_directory(&data_dir).unwrap();
        assert_eq!(count, 2);

        let results = engine.search_indexed("hello", 10);
        assert!(!results.is_empty());
        assert!(results[0].name.contains("hello"));
    }
}
