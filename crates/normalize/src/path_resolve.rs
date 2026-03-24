use glob::Pattern as GlobPattern;
use std::path::Path;

use crate::config::NormalizeConfig;
use crate::skeleton::SkeletonExtractor;
use normalize_languages::Symbol;

// Re-export public types from the extracted crate
pub use normalize_path_resolve::{
    PathEntry, PathMatch, PathMatchKind, PathSource, SigilExpansion, UnifiedPath,
};

/// Wraps pre-fetched index file data as a `PathSource`.
///
/// Data is fetched once during `open` (or `open_sync`). The sync `PathSource`
/// methods then operate on cached data — no stored `Runtime` needed.
struct IndexPathSource {
    files: Vec<(String, bool)>,
}

impl IndexPathSource {
    async fn open(root: &Path) -> Option<Self> {
        let mut index = crate::index::open_if_enabled(root).await?;
        let _ = index.incremental_refresh().await;
        let files = index
            .all_files()
            .await
            .ok()?
            .into_iter()
            .map(|f| (f.path, f.is_dir))
            .collect();
        Some(Self { files })
    }

    /// Open from a sync context: uses `block_in_place` inside a tokio runtime,
    /// or creates a temporary runtime when called outside tokio.
    fn open_sync(root: &Path) -> Option<Self> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(Self::open(root))),
            Err(_) => tokio::runtime::Runtime::new()
                .ok()?
                .block_on(Self::open(root)),
        }
    }
}

impl PathSource for IndexPathSource {
    fn find_like(&self, query: &str) -> Option<Vec<normalize_path_resolve::PathEntry>> {
        let q = query.to_lowercase();
        Some(
            self.files
                .iter()
                .filter(|(p, _)| p.to_lowercase().contains(&q))
                .map(|(path, is_dir)| normalize_path_resolve::PathEntry {
                    path: path.clone(),
                    kind: if *is_dir {
                        normalize_path_resolve::PathMatchKind::Directory
                    } else {
                        normalize_path_resolve::PathMatchKind::File
                    },
                })
                .collect(),
        )
    }

    fn all_files(&self) -> Option<Vec<normalize_path_resolve::PathEntry>> {
        Some(
            self.files
                .iter()
                .map(|(path, is_dir)| normalize_path_resolve::PathEntry {
                    path: path.clone(),
                    kind: if *is_dir {
                        normalize_path_resolve::PathMatchKind::Directory
                    } else {
                        normalize_path_resolve::PathMatchKind::File
                    },
                })
                .collect(),
        )
    }
}

fn alias_lookup(root: &Path) -> impl Fn(&str) -> Option<Vec<String>> {
    let config = NormalizeConfig::load(root);
    move |name: &str| config.aliases.get(name)
}

/// Expand an alias query like `@todo` or `@config/section`.
pub fn expand_sigil(query: &str, root: &Path) -> Option<SigilExpansion> {
    normalize_path_resolve::expand_sigil(query, &alias_lookup(root))
}

/// Resolve a unified path like `src/main.py/Foo/bar` to file + symbol components.
pub fn resolve_unified(query: &str, root: &Path) -> Option<UnifiedPath> {
    normalize_path_resolve::resolve_unified(
        query,
        root,
        &alias_lookup(root),
        IndexPathSource::open_sync(root)
            .as_ref()
            .map(|s| s as &dyn PathSource),
    )
}

/// Resolve a query to ALL matching unified paths.
pub fn resolve_unified_all(query: &str, root: &Path) -> Vec<UnifiedPath> {
    normalize_path_resolve::resolve_unified_all(
        query,
        root,
        &alias_lookup(root),
        IndexPathSource::open_sync(root)
            .as_ref()
            .map(|s| s as &dyn PathSource),
    )
}

/// Get all files in the repository.
pub fn all_files(root: &Path) -> Vec<PathMatch> {
    normalize_path_resolve::all_files(
        root,
        IndexPathSource::open_sync(root)
            .as_ref()
            .map(|s| s as &dyn PathSource),
    )
}

/// Resolve a fuzzy query to matching paths.
pub fn resolve(query: &str, root: &Path) -> Vec<PathMatch> {
    normalize_path_resolve::resolve(
        query,
        root,
        IndexPathSource::open_sync(root)
            .as_ref()
            .map(|s| s as &dyn PathSource),
    )
}

/// Check if a pattern contains glob characters (* ? [)
pub use normalize_path_resolve::is_glob_pattern;

/// A symbol match with its full path (for glob resolution)
#[derive(Debug, Clone)]
pub struct SymbolMatch {
    /// The symbol itself
    pub symbol: Symbol,
    /// Full path from root (e.g., "Parent/Child/Symbol")
    pub path: String,
}

/// Resolve symbols matching a glob pattern within a file.
pub fn resolve_symbol_glob(file_path: &Path, content: &str, pattern: &str) -> Vec<SymbolMatch> {
    let extractor = SkeletonExtractor::new();
    let result = extractor.extract(file_path, content);

    let glob = match GlobPattern::new(pattern) {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };

    fn collect_matching(
        symbols: &[Symbol],
        glob: &GlobPattern,
        parent_path: &str,
        matches: &mut Vec<SymbolMatch>,
    ) {
        for sym in symbols {
            let sym_path = if parent_path.is_empty() {
                sym.name.clone()
            } else {
                format!("{}/{}", parent_path, sym.name)
            };

            if glob.matches(&sym_path) {
                matches.push(SymbolMatch {
                    symbol: sym.clone(),
                    path: sym_path.clone(),
                });
            }

            // Recurse into children
            collect_matching(&sym.children, glob, &sym_path, matches);
        }
    }

    let mut matches = Vec::new();
    collect_matching(&result.symbols, &glob, "", &mut matches);

    // Sort by line number (ascending, for display purposes)
    matches.sort_by_key(|m| m.symbol.start_line);
    matches
}
