use ignore::WalkBuilder;
use moss_core::get_moss_dir;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::symbols::SymbolParser;

#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    pub path: String,
    pub kind: String,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallerInfo {
    pub file: String,
    pub symbol: String,
    pub line: u32,
}

pub struct SymbolIndex {
    conn: Connection,
    root: PathBuf,
    parser: SymbolParser,
}

impl SymbolIndex {
    pub fn open(root: &Path) -> rusqlite::Result<Self> {
        let moss_dir = get_moss_dir(root);
        std::fs::create_dir_all(&moss_dir).ok();

        let db_path = moss_dir.join("symbols.sqlite");
        let conn = Connection::open(&db_path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;

             CREATE TABLE IF NOT EXISTS files (
                 path TEXT PRIMARY KEY,
                 is_dir INTEGER NOT NULL,
                 mtime INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS symbols (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 kind TEXT NOT NULL,
                 file TEXT NOT NULL,
                 start_line INTEGER NOT NULL,
                 end_line INTEGER NOT NULL,
                 parent TEXT,
                 FOREIGN KEY (file) REFERENCES files(path) ON DELETE CASCADE
             );

             CREATE TABLE IF NOT EXISTS calls (
                 id INTEGER PRIMARY KEY,
                 caller_file TEXT NOT NULL,
                 caller_symbol TEXT NOT NULL,
                 caller_line INTEGER NOT NULL,
                 callee_name TEXT NOT NULL,
                 FOREIGN KEY (caller_file) REFERENCES files(path) ON DELETE CASCADE
             );

             CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
             CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file);
             CREATE INDEX IF NOT EXISTS idx_calls_callee ON calls(callee_name);
             CREATE INDEX IF NOT EXISTS idx_calls_caller ON calls(caller_file, caller_symbol);
            "
        )?;

        Ok(Self {
            conn,
            root: root.to_path_buf(),
            parser: SymbolParser::new(),
        })
    }

    pub fn file_count(&self) -> rusqlite::Result<usize> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM files WHERE is_dir = 0",
            [],
            |row| row.get(0),
        )
    }

    pub fn symbol_count(&self) -> rusqlite::Result<usize> {
        self.conn.query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))
    }

    pub fn full_reindex(&mut self) -> rusqlite::Result<usize> {
        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        self.conn.execute("DELETE FROM calls", [])?;
        self.conn.execute("DELETE FROM symbols", [])?;
        self.conn.execute("DELETE FROM files", [])?;

        let mut count = 0;
        let mut files_to_index: Vec<(String, PathBuf)> = Vec::new();

        for entry in walker.flatten() {
            let path = entry.path();
            if let Ok(rel) = path.strip_prefix(&self.root) {
                let rel_str = rel.to_string_lossy().to_string();
                if rel_str.is_empty() || rel_str.starts_with(".moss") {
                    continue;
                }

                let is_dir = path.is_dir();
                let mtime = path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                self.conn.execute(
                    "INSERT INTO files (path, is_dir, mtime) VALUES (?1, ?2, ?3)",
                    params![rel_str, is_dir as i64, mtime],
                )?;

                if !is_dir && (rel_str.ends_with(".py") || rel_str.ends_with(".rs")) {
                    files_to_index.push((rel_str, path.to_path_buf()));
                }

                count += 1;
            }
        }

        // Index symbols and calls
        for (rel_path, abs_path) in files_to_index {
            if let Ok(content) = std::fs::read_to_string(&abs_path) {
                self.index_content(&rel_path, &abs_path, &content)?;
            }
        }

        Ok(count)
    }

    pub fn index_file(&mut self, path: &Path) -> rusqlite::Result<()> {
        let rel = match path.strip_prefix(&self.root) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => return Ok(()),
        };

        if rel.starts_with(".moss") {
            return Ok(());
        }

        let is_dir = path.is_dir();
        let mtime = path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Remove old data
        self.conn.execute("DELETE FROM calls WHERE caller_file = ?1", params![rel])?;
        self.conn.execute("DELETE FROM symbols WHERE file = ?1", params![rel])?;
        self.conn.execute("DELETE FROM files WHERE path = ?1", params![rel])?;

        // Insert new file entry
        self.conn.execute(
            "INSERT INTO files (path, is_dir, mtime) VALUES (?1, ?2, ?3)",
            params![rel, is_dir as i64, mtime],
        )?;

        // Index symbols if it's a supported file type
        if !is_dir && (rel.ends_with(".py") || rel.ends_with(".rs")) {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.index_content(&rel, path, &content)?;
            }
        }

        Ok(())
    }

    fn index_content(&mut self, rel_path: &str, abs_path: &Path, content: &str) -> rusqlite::Result<()> {
        // Parse symbols
        let symbols = self.parser.parse_file(abs_path, content);
        for sym in &symbols {
            self.conn.execute(
                "INSERT INTO symbols (name, kind, file, start_line, end_line, parent)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    sym.name,
                    sym.kind.as_str(),
                    rel_path,
                    sym.start_line,
                    sym.end_line,
                    sym.parent
                ],
            )?;

            // Find calls within this symbol
            if let Some(source) = self.parser.extract_symbol_source(abs_path, content, &sym.name) {
                let callees = self.parser.find_calls_in_source(&source);
                for (callee, line_offset) in callees {
                    self.conn.execute(
                        "INSERT INTO calls (caller_file, caller_symbol, caller_line, callee_name)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            rel_path,
                            sym.name,
                            sym.start_line + line_offset,
                            callee
                        ],
                    )?;
                }
            }
        }

        Ok(())
    }

    pub fn remove_file(&mut self, path: &Path) -> rusqlite::Result<()> {
        let rel = match path.strip_prefix(&self.root) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => return Ok(()),
        };

        self.conn.execute("DELETE FROM calls WHERE caller_file = ?1", params![rel])?;
        self.conn.execute("DELETE FROM symbols WHERE file = ?1", params![rel])?;
        self.conn.execute("DELETE FROM files WHERE path = ?1", params![rel])?;

        Ok(())
    }

    pub fn resolve_path(&self, query: &str) -> rusqlite::Result<Vec<FileMatch>> {
        // Exact match first
        let mut stmt = self.conn.prepare(
            "SELECT path, is_dir FROM files WHERE path = ?1"
        )?;
        let exact: Vec<FileMatch> = stmt
            .query_map(params![query], |row| {
                let is_dir: i64 = row.get(1)?;
                Ok(FileMatch {
                    path: row.get(0)?,
                    kind: if is_dir != 0 { "directory" } else { "file" }.to_string(),
                    score: 1000,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        if !exact.is_empty() {
            return Ok(exact);
        }

        // Filename match
        let pattern = format!("%/{}", query);
        let mut stmt = self.conn.prepare(
            "SELECT path, is_dir FROM files WHERE path LIKE ?1 OR path = ?2 LIMIT 20"
        )?;
        let matches: Vec<FileMatch> = stmt
            .query_map(params![pattern, query], |row| {
                let is_dir: i64 = row.get(1)?;
                Ok(FileMatch {
                    path: row.get(0)?,
                    kind: if is_dir != 0 { "directory" } else { "file" }.to_string(),
                    score: 500,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        if !matches.is_empty() {
            return Ok(matches);
        }

        // Fuzzy match using LIKE with wildcards between characters
        let fuzzy_pattern: String = query
            .chars()
            .map(|c| format!("%{}", c))
            .collect::<String>() + "%";

        let mut stmt = self.conn.prepare(
            "SELECT path, is_dir FROM files WHERE path LIKE ?1 LIMIT 20"
        )?;
        let matches: Vec<FileMatch> = stmt
            .query_map(params![fuzzy_pattern], |row| {
                let is_dir: i64 = row.get(1)?;
                Ok(FileMatch {
                    path: row.get(0)?,
                    kind: if is_dir != 0 { "directory" } else { "file" }.to_string(),
                    score: 100,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(matches)
    }

    pub fn get_symbols(&self, file: &str) -> rusqlite::Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file, start_line, end_line, parent
             FROM symbols WHERE file = ?1 OR file LIKE ?2
             ORDER BY start_line"
        )?;
        let pattern = format!("%/{}", file);
        let symbols: Vec<Symbol> = stmt
            .query_map(params![file, pattern], |row| {
                Ok(Symbol {
                    name: row.get(0)?,
                    kind: row.get(1)?,
                    file: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    parent: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(symbols)
    }

    pub fn find_callers(&self, symbol: &str) -> rusqlite::Result<Vec<CallerInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT caller_file, caller_symbol, caller_line
             FROM calls WHERE callee_name = ?1
             ORDER BY caller_file, caller_line"
        )?;
        let callers: Vec<CallerInfo> = stmt
            .query_map(params![symbol], |row| {
                Ok(CallerInfo {
                    file: row.get(0)?,
                    symbol: row.get(1)?,
                    line: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(callers)
    }

    pub fn find_callees(&self, symbol: &str, file: &str) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT callee_name FROM calls
             WHERE caller_symbol = ?1 AND (caller_file = ?2 OR caller_file LIKE ?3)
             ORDER BY callee_name"
        )?;
        let pattern = format!("%/{}", file);
        let callees: Vec<String> = stmt
            .query_map(params![symbol, file, pattern], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(callees)
    }

    pub fn expand_symbol(&self, symbol: &str, file: Option<&str>) -> rusqlite::Result<String> {
        let sym = if let Some(f) = file {
            let pattern = format!("%/{}", f);
            self.conn.query_row(
                "SELECT file, start_line, end_line FROM symbols
                 WHERE name = ?1 AND (file = ?2 OR file LIKE ?3)",
                params![symbol, f, pattern],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?, row.get::<_, u32>(2)?)),
            )
        } else {
            self.conn.query_row(
                "SELECT file, start_line, end_line FROM symbols WHERE name = ?1",
                params![symbol],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?, row.get::<_, u32>(2)?)),
            )
        };

        match sym {
            Ok((file_path, start, end)) => {
                let abs_path = self.root.join(&file_path);
                if let Ok(content) = std::fs::read_to_string(&abs_path) {
                    let lines: Vec<&str> = content.lines().collect();
                    let start_idx = (start as usize).saturating_sub(1);
                    let end_idx = (end as usize).min(lines.len());
                    let source = lines[start_idx..end_idx].join("\n");
                    Ok(source)
                } else {
                    Err(rusqlite::Error::QueryReturnedNoRows)
                }
            }
            Err(e) => Err(e),
        }
    }
}
