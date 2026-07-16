//! Ad-hoc driver for the `normalize-semantic-facts` prototype: walks a
//! directory, extracts facts from every `.ts`/`.tsx`/`.sql` file it finds,
//! and prints a restatement + similarity report. Not a real CLI — just
//! enough to exercise the extractors and restatement finder against real
//! repositories.

use std::fs;
use std::path::Path;

use normalize_semantic_facts::{
    FactExtractor, FactOccurrence, SqlExtractor, TypeScriptExtractor, extract_from_source,
    find_restatements, find_similar, restated_only,
};
use walkdir::{DirEntry, WalkDir};

const SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", "dist", "build"];

fn is_skipped(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| SKIP_DIRS.contains(&name))
            .unwrap_or(false)
}

fn grammar_for(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|e| e.to_str())? {
        "ts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "sql" => Some("sql"),
        _ => None,
    }
}

fn extract_file(path: &Path, root: &Path) -> Vec<FactOccurrence> {
    let Some(grammar) = grammar_for(path) else {
        return Vec::new();
    };
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let rel = path.strip_prefix(root).unwrap_or(path);
    let file = rel.to_string_lossy().to_string();

    match grammar {
        "sql" => extract_from_source(&SqlExtractor, &source, &file).unwrap_or_default(),
        // "typescript" and "tsx" both lower via TypeScriptExtractor's CST
        // walk (the tsx grammar is a superset of typescript's node types);
        // extract_from_source hardcodes grammar_name() to "typescript", so
        // for .tsx files we parse with the "tsx" grammar directly and call
        // extract() ourselves.
        "tsx" => {
            let Some(tree) = normalize_languages::parsers::parse_with_grammar("tsx", &source)
            else {
                return Vec::new();
            };
            TypeScriptExtractor.extract(&tree, &source, &file)
        }
        _ => extract_from_source(&TypeScriptExtractor, &source, &file).unwrap_or_default(),
    }
}

fn walk_and_extract(root: &Path) -> Vec<FactOccurrence> {
    let mut occurrences = Vec::new();
    let mut files_seen = 0usize;
    let mut ext_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_skipped(e))
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            *ext_counts.entry(ext.to_string()).or_default() += 1;
        }
        if grammar_for(path).is_some() {
            files_seen += 1;
            occurrences.extend(extract_file(path, root));
        }
    }

    eprintln!(
        "  ({files_seen} .ts/.tsx/.sql files parsed, {} facts extracted)",
        occurrences.len()
    );
    let mut top_exts: Vec<(String, usize)> = ext_counts.into_iter().collect();
    top_exts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    top_exts.truncate(15);
    eprintln!(
        "  (top file extensions seen: {})",
        top_exts
            .iter()
            .map(|(e, c)| format!(".{e}={c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    occurrences
}

fn print_report(root: &Path) {
    println!("=== {} ===", root.display());
    let occurrences = walk_and_extract(root);
    if occurrences.is_empty() {
        println!("(no facts extracted)\n");
        return;
    }

    println!("\n--- Restated facts (exact IR equality, count desc) ---");
    let restated = restated_only(find_restatements(&occurrences));
    if restated.is_empty() {
        println!("(none)");
    }
    for group in &restated {
        println!("\n[{}x] {:?}", group.count(), group.fact);
        for loc in &group.locations {
            println!("    {}:{}", loc.file, loc.line);
        }
    }

    println!("\n--- Similar groups (identity match, type relations classified) ---");
    let similar = find_similar(&occurrences);
    let interesting: Vec<_> = similar
        .iter()
        .filter(|g| g.entries.len() > 1 || !g.relations.is_empty())
        .collect();
    if interesting.is_empty() {
        println!("(none — every identity key has at most one distinct type shape)");
    }
    for group in &interesting {
        println!(
            "\n[{} total occurrences, {} distinct shapes]",
            group.total_count(),
            group.entries.len()
        );
        for (i, entry) in group.entries.iter().enumerate() {
            println!("  #{i} [{}x] {:?}", entry.count(), entry.fact);
            for loc in &entry.locations {
                println!("      {}:{}", loc.file, loc.line);
            }
        }
        for rel in &group.relations {
            println!("  relation(#{}, #{}) = {:?}", rel.a, rel.b, rel.relation);
        }
    }
    println!();
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: facts-run <directory>");
        std::process::exit(1);
    });
    let root = Path::new(&arg);
    if !root.is_dir() {
        eprintln!("not a directory: {}", root.display());
        std::process::exit(1);
    }
    print_report(root);
}
