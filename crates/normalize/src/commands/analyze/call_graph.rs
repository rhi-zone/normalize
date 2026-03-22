//! Call graph analysis - show callers/callees of symbols

use crate::index;
use crate::path_resolve;

/// A single entry in the call graph result.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct CallEntry {
    pub file: String,
    pub symbol: String,
    pub line: usize,
    pub direction: String, // "caller" or "callee"
}

/// Build the call graph results without printing (for service layer).
pub async fn build_call_graph(
    root: &std::path::Path,
    target: &str,
    show_callers: bool,
    show_callees: bool,
    case_insensitive: bool,
) -> Result<Vec<CallEntry>, String> {
    let (symbol, file_hint) = if let Some((sym, file)) = parse_file_symbol_string(target) {
        (sym, Some(file))
    } else {
        (target.to_string(), None)
    };

    let idx = index::ensure_ready(root).await?;

    let stats = idx.call_graph_stats().await.unwrap_or_default();
    if stats.calls == 0 {
        return Err("Call graph not indexed. Run: normalize structure rebuild".to_string());
    }

    let _ = case_insensitive; // Index methods already have case-insensitive fallbacks

    // Resolve def_file from hint or index lookup.
    let def_file = if let Some(f) = &file_hint {
        let matches = path_resolve::resolve(f, root);
        matches
            .iter()
            .find(|m| m.kind == "file")
            .map(|m| m.path.clone())
    } else {
        idx.find_symbol(&symbol)
            .await
            .ok()
            .and_then(|syms| syms.first().map(|(f, _, _, _)| f.clone()))
    };

    let def_file = def_file.ok_or_else(|| {
        format!(
            "Symbol '{}' not found in index. Try: file/Symbol format.",
            symbol
        )
    })?;

    let mut results: Vec<CallEntry> = Vec::new();

    if show_callers {
        match idx.find_callers(&symbol, &def_file).await {
            Ok(callers) => {
                for (file, sym, line) in callers {
                    results.push(CallEntry {
                        file,
                        symbol: sym,
                        line,
                        direction: "caller".to_string(),
                    });
                }
            }
            Err(e) => {
                eprintln!("Error finding callers: {}", e);
            }
        }
    }

    if show_callees {
        match idx.find_callees(&def_file, &symbol).await {
            Ok(callees) => {
                for (name, line) in callees {
                    results.push(CallEntry {
                        file: def_file.clone(),
                        symbol: name,
                        line,
                        direction: "callee".to_string(),
                    });
                }
            }
            Err(e) => {
                eprintln!("Error finding callees: {}", e);
            }
        }
    }

    results.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

    Ok(results)
}

/// Try various separators to parse file:symbol format
fn parse_file_symbol_string(s: &str) -> Option<(String, String)> {
    // Try various separators: #, ::, :
    for sep in ["#", "::", ":"] {
        if let Some(idx) = s.find(sep) {
            let (file, rest) = s.split_at(idx);
            let symbol = &rest[sep.len()..];
            if !file.is_empty() && !symbol.is_empty() && looks_like_file(file) {
                return Some((symbol.to_string(), file.to_string()));
            }
        }
    }
    None
}

/// Check if a string looks like a file path
fn looks_like_file(s: &str) -> bool {
    s.contains('.') || s.contains('/')
}
