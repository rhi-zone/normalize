use glob::Pattern as GlobPattern;
use std::path::Path;

use crate::config::NormalizeConfig;
use crate::skeleton::{SkeletonExtractor, SkeletonSymbol};

// Re-export public types from the extracted crate
pub use normalize_path_resolve::{PathMatch, PathSource, SigilExpansion, UnifiedPath};

/// Wraps a `FileIndex` + `tokio::Runtime` as a `PathSource`.
struct IndexPathSource {
    index: crate::index::FileIndex,
    rt: tokio::runtime::Runtime,
}

impl IndexPathSource {
    fn open(root: &Path) -> Option<Self> {
        let rt = tokio::runtime::Runtime::new().ok()?;
        let mut index = rt.block_on(crate::index::open_if_enabled(root))?;
        let _ = rt.block_on(index.incremental_refresh());
        Some(Self { index, rt })
    }
}

impl PathSource for IndexPathSource {
    fn find_like(&mut self, query: &str) -> Option<Vec<(String, bool)>> {
        self.rt
            .block_on(self.index.find_like(query))
            .ok()
            .map(|files| files.into_iter().map(|f| (f.path, f.is_dir)).collect())
    }

    fn all_files(&mut self) -> Option<Vec<(String, bool)>> {
        self.rt
            .block_on(self.index.all_files())
            .ok()
            .map(|files| files.into_iter().map(|f| (f.path, f.is_dir)).collect())
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
        IndexPathSource::open(root)
            .as_mut()
            .map(|s| s as &mut dyn PathSource),
    )
}

/// Resolve a query to ALL matching unified paths.
pub fn resolve_unified_all(query: &str, root: &Path) -> Vec<UnifiedPath> {
    normalize_path_resolve::resolve_unified_all(
        query,
        root,
        &alias_lookup(root),
        IndexPathSource::open(root)
            .as_mut()
            .map(|s| s as &mut dyn PathSource),
    )
}

/// Get all files in the repository.
pub fn all_files(root: &Path) -> Vec<PathMatch> {
    normalize_path_resolve::all_files(
        root,
        IndexPathSource::open(root)
            .as_mut()
            .map(|s| s as &mut dyn PathSource),
    )
}

/// Resolve a fuzzy query to matching paths.
pub fn resolve(query: &str, root: &Path) -> Vec<PathMatch> {
    normalize_path_resolve::resolve(
        query,
        root,
        IndexPathSource::open(root)
            .as_mut()
            .map(|s| s as &mut dyn PathSource),
    )
}

/// Check if a pattern contains glob characters (* ? [)
pub use normalize_path_resolve::is_glob_pattern;

/// A symbol match with its full path (for glob resolution)
#[derive(Debug, Clone)]
pub struct SymbolMatch {
    /// The symbol itself
    pub symbol: SkeletonSymbol,
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
        symbols: &[SkeletonSymbol],
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
