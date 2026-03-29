//! LSP (Language Server Protocol) server for normalize.
//!
//! Provides IDE integration with document symbols, workspace symbols, hover,
//! and diagnostics from syntax/fact rule engines.

use crate::index::FileIndex;
use crate::skeleton::SkeletonExtractor;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

/// Normalize LSP backend.
struct NormalizeBackend {
    client: Client,
    root: Mutex<Option<PathBuf>>,
    index: Mutex<Option<FileIndex>>,
    /// Persistent extractor — avoids recreating grammar caches per request.
    extractor: SkeletonExtractor,
    /// Files with syntax diagnostics in the last per-file run.
    syntax_diagnosed_files: Arc<Mutex<HashSet<Url>>>,
    /// Files with fact diagnostics in the last workspace-wide run.
    fact_diagnosed_files: Arc<Mutex<HashSet<Url>>>,
    /// Generation counter for debouncing fact diagnostic runs.
    fact_diagnostics_generation: Arc<std::sync::atomic::AtomicU64>,
    /// Debounce interval for fact diagnostics in milliseconds.
    fact_debounce_ms: std::sync::atomic::AtomicU64,
}

impl NormalizeBackend {
    fn new(client: Client) -> Self {
        Self {
            client,
            root: Mutex::new(None),
            index: Mutex::new(None),
            extractor: SkeletonExtractor::new(),
            syntax_diagnosed_files: Arc::new(Mutex::new(HashSet::new())),
            fact_diagnosed_files: Arc::new(Mutex::new(HashSet::new())),
            fact_diagnostics_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            fact_debounce_ms: std::sync::atomic::AtomicU64::new(
                super::ServeConfig::default().fact_debounce_ms(),
            ),
        }
    }

    /// Initialize index for the workspace root.
    async fn init_index(&self, root: PathBuf) {
        if let Some(idx) = crate::index::open_if_enabled(&root).await {
            *self.index.lock().await = Some(idx);
        }

        // Load configurable debounce from project config
        let config = crate::config::NormalizeConfig::load(&root);
        self.fact_debounce_ms.store(
            config.serve.fact_debounce_ms(),
            std::sync::atomic::Ordering::Relaxed,
        );

        *self.root.lock().await = Some(root.clone());

        // Run initial diagnostics (all rules, no debounce)
        self.schedule_all_diagnostics().await;
    }

    /// Schedule all diagnostics (syntax + fact) for initial load.
    async fn schedule_all_diagnostics(&self) {
        let root = self.root.lock().await.clone();
        let Some(root) = root else { return };

        let client = self.client.clone();
        let syntax_diagnosed = Arc::clone(&self.syntax_diagnosed_files);
        let fact_diagnosed = Arc::clone(&self.fact_diagnosed_files);

        tokio::spawn(async move {
            run_and_publish_diagnostics(
                &client,
                &root,
                &normalize_rules::RuleKind::Syntax,
                &syntax_diagnosed,
            )
            .await;
            run_and_publish_diagnostics(
                &client,
                &root,
                &normalize_rules::RuleKind::Fact,
                &fact_diagnosed,
            )
            .await;
        });
    }

    /// Run syntax diagnostics immediately for a single file.
    async fn run_syntax_diagnostics_for_file(&self, uri: &Url) {
        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        let root = self.root.lock().await.clone();
        let Some(root) = root else { return };

        let client = self.client.clone();
        let syntax_diagnosed = Arc::clone(&self.syntax_diagnosed_files);
        let file_owned = file_path.clone();
        let root_owned = root.clone();

        tokio::spawn(async move {
            let report = tokio::task::spawn_blocking(move || {
                let config = crate::config::NormalizeConfig::load(&root_owned);
                let rules_config = normalize_rules::RulesRunConfig {
                    rule_tags: config.rule_tags.0.clone(),
                    rules: config.rules.clone(),
                };
                normalize_rules::run_rules_report(
                    &file_owned,
                    &root_owned,
                    None,
                    None,
                    &normalize_rules::RuleKind::Syntax,
                    &[],
                    &rules_config,
                    None,
                    &normalize_rules_config::PathFilter::default(),
                )
            })
            .await;

            let report = match report {
                Ok(r) => r,
                Err(e) => {
                    client
                        .log_message(
                            MessageType::ERROR,
                            format!("Failed to run syntax diagnostics: {e}"),
                        )
                        .await;
                    return;
                }
            };

            let diagnostics: Vec<Diagnostic> =
                report.issues.iter().map(issue_to_lsp_diagnostic).collect();

            let uri = match Url::from_file_path(&file_path) {
                Ok(u) => u,
                Err(_) => return,
            };

            let mut prev = syntax_diagnosed.lock().await;
            if diagnostics.is_empty() {
                // Clear syntax diagnostics for this file if it had them before
                if prev.remove(&uri) {
                    client.publish_diagnostics(uri, vec![], None).await;
                }
            } else {
                client
                    .publish_diagnostics(uri.clone(), diagnostics, None)
                    .await;
                prev.insert(uri);
            }
        });
    }

    /// Schedule debounced fact diagnostics (1500ms delay, workspace-wide).
    /// Incrementally updates the index for the saved file before running fact rules.
    async fn schedule_fact_diagnostics(&self, saved_uri: &Url) {
        let generation = self
            .fact_diagnostics_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;

        let root = self.root.lock().await.clone();
        let Some(root) = root else { return };

        // Compute relative path for index update
        let rel_path = saved_uri.to_file_path().ok().and_then(|p| {
            p.strip_prefix(&root)
                .ok()
                .map(|r| r.to_string_lossy().to_string())
        });

        let client = self.client.clone();
        let fact_diagnosed = Arc::clone(&self.fact_diagnosed_files);
        let gen_ref = Arc::clone(&self.fact_diagnostics_generation);
        let debounce_ms = self
            .fact_debounce_ms
            .load(std::sync::atomic::Ordering::Relaxed);

        tokio::spawn(async move {
            // Debounce: wait configured interval, then check if we're still the latest request
            tokio::time::sleep(std::time::Duration::from_millis(debounce_ms)).await;
            let current = gen_ref.load(std::sync::atomic::Ordering::SeqCst);
            if current != generation {
                return; // superseded by a newer request
            }

            // Incrementally update the index for the saved file
            if let Some(rel) = &rel_path
                && let Ok(mut idx) = crate::index::open(&root).await
                && let Err(e) = idx.update_file(rel).await
            {
                client
                    .log_message(MessageType::WARNING, format!("Index update for {rel}: {e}"))
                    .await;
            }

            run_and_publish_diagnostics(
                &client,
                &root,
                &normalize_rules::RuleKind::Fact,
                &fact_diagnosed,
            )
            .await;
        });
    }

    /// Convert normalize symbol kind to LSP SymbolKind.
    fn to_lsp_symbol_kind(kind: &str) -> SymbolKind {
        match kind {
            "class" | "struct" => SymbolKind::CLASS,
            "function" => SymbolKind::FUNCTION,
            "method" => SymbolKind::METHOD,
            "interface" | "trait" => SymbolKind::INTERFACE,
            "enum" => SymbolKind::ENUM,
            "constant" | "const" => SymbolKind::CONSTANT,
            "variable" | "field" => SymbolKind::VARIABLE,
            "property" => SymbolKind::PROPERTY,
            "module" => SymbolKind::MODULE,
            "type" | "type_alias" => SymbolKind::TYPE_PARAMETER,
            "namespace" => SymbolKind::NAMESPACE,
            _ => SymbolKind::VARIABLE,
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for NormalizeBackend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Get workspace root from params
        if let Some(root_uri) = params.root_uri
            && let Ok(path) = root_uri.to_file_path()
        {
            self.init_index(path).await;
        } else if let Some(folders) = params.workspace_folders
            && let Some(folder) = folders.first()
            && let Ok(path) = folder.uri.to_file_path()
        {
            self.init_index(path).await;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(false),
                        })),
                        ..Default::default()
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "normalize".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "normalize LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = &params.text_document.uri;
        // Fast: per-file syntax diagnostics (immediate)
        self.run_syntax_diagnostics_for_file(uri).await;
        // Slow: workspace-wide fact diagnostics (debounced 1500ms)
        self.schedule_fact_diagnostics(uri).await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        // Read file content
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        // Extract symbols using persistent extractor
        let result = self.extractor.extract(&file_path, &content);

        // Convert to LSP document symbols (nested structure)
        fn to_document_symbol(sym: &normalize_languages::Symbol) -> DocumentSymbol {
            let range = Range {
                start: Position {
                    line: sym.start_line.saturating_sub(1) as u32,
                    character: 0,
                },
                end: Position {
                    line: sym.end_line.saturating_sub(1) as u32,
                    character: 0,
                },
            };

            let children: Vec<DocumentSymbol> =
                sym.children.iter().map(to_document_symbol).collect();

            #[allow(deprecated)]
            DocumentSymbol {
                name: sym.name.clone(),
                detail: if sym.signature.is_empty() {
                    None
                } else {
                    Some(sym.signature.clone())
                },
                kind: NormalizeBackend::to_lsp_symbol_kind(sym.kind.as_str()),
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: if children.is_empty() {
                    None
                } else {
                    Some(children)
                },
            }
        }

        let symbols: Vec<DocumentSymbol> = result.symbols.iter().map(to_document_symbol).collect();

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = &params.query;

        let index = self.index.lock().await;
        let root = self.root.lock().await;

        let (index, root) = match (index.as_ref(), root.as_ref()) {
            (Some(i), Some(r)) => (i, r.clone()),
            _ => return Ok(None),
        };

        // Search symbols in index
        let matches = match index.find_symbols(query, None, false, 50).await {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        #[allow(deprecated)]
        let symbols: Vec<SymbolInformation> = matches
            .into_iter()
            .map(|sym| {
                let file_path = root.clone().join(&sym.file);
                let uri = Url::from_file_path(&file_path)
                    // normalize-syntax-allow: rust/unwrap-in-impl - "file:///unknown" is a compile-time constant valid URL
                    .unwrap_or_else(|_| Url::parse("file:///unknown").unwrap());

                SymbolInformation {
                    name: sym.name,
                    kind: Self::to_lsp_symbol_kind(&sym.kind),
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri,
                        range: Range {
                            start: Position {
                                line: sym.start_line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: sym.end_line.saturating_sub(1) as u32,
                                character: 0,
                            },
                        },
                    },
                    container_name: None,
                }
            })
            .collect();

        Ok(Some(symbols))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        // Read file content
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        // Extract symbols using persistent extractor
        let result = self.extractor.extract(&file_path, &content);

        // Find symbol at position (1-indexed line)
        let line = position.line as usize + 1;

        fn find_symbol_at_line(
            symbols: &[normalize_languages::Symbol],
            line: usize,
        ) -> Option<&normalize_languages::Symbol> {
            for sym in symbols {
                if line >= sym.start_line && line <= sym.end_line {
                    // Check children first for more specific match
                    if let Some(child) = find_symbol_at_line(&sym.children, line) {
                        return Some(child);
                    }
                    return Some(sym);
                }
            }
            None
        }

        let symbol = find_symbol_at_line(&result.symbols, line);

        match symbol {
            Some(sym) => {
                let mut content = format!("**{}** `{}`", sym.kind.as_str(), sym.name);
                if !sym.signature.is_empty() {
                    content.push_str(&format!("\n\n```\n{}\n```", sym.signature));
                }
                if let Some(doc) = &sym.docstring {
                    content.push_str(&format!("\n\n{}", doc));
                }

                Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: content,
                    }),
                    range: Some(Range {
                        start: Position {
                            line: sym.start_line.saturating_sub(1) as u32,
                            character: 0,
                        },
                        end: Position {
                            line: sym.end_line.saturating_sub(1) as u32,
                            character: 0,
                        },
                    }),
                }))
            }
            None => Ok(None),
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        // Read file content to get the word at position
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        // Get the word at the cursor position
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return Ok(None);
        }

        let line = lines[line_idx];
        let col = position.character as usize;

        // Find word boundaries
        let word = extract_word_at_position(line, col);
        if word.is_empty() {
            return Ok(None);
        }

        // Search for symbol definition in index
        let index = self.index.lock().await;
        let root = self.root.lock().await;

        let (index, root) = match (index.as_ref(), root.as_ref()) {
            (Some(i), Some(r)) => (i, r.clone()),
            _ => return Ok(None),
        };

        // Look up symbol in index
        let matches = match index.find_symbol(&word).await {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        if matches.is_empty() {
            return Ok(None);
        }

        // Return first match (could enhance to return all)
        let (file, _kind, start_line, _end_line) = &matches[0];
        let target_path = root.join(file);
        let target_uri = match Url::from_file_path(&target_path) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: target_uri,
            range: Range {
                start: Position {
                    line: start_line.saturating_sub(1) as u32,
                    character: 0,
                },
                end: Position {
                    line: start_line.saturating_sub(1) as u32,
                    character: 0,
                },
            },
        })))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        // Read file content to get the word at position
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return Ok(None);
        }

        let line = lines[line_idx];
        let col = position.character as usize;
        let word = extract_word_at_position(line, col);
        if word.is_empty() {
            return Ok(None);
        }

        let index = self.index.lock().await;
        let root = self.root.lock().await;

        let (index, root) = match (index.as_ref(), root.as_ref()) {
            (Some(i), Some(r)) => (i, r.clone()),
            _ => return Ok(None),
        };

        let mut locations = Vec::new();

        // Include definition if requested
        if params.context.include_declaration
            && let Ok(defs) = index.find_symbol(&word).await
        {
            for (file, _kind, start_line, _end_line) in defs {
                if let Some(loc) = make_location_at_line(&file, &root, start_line) {
                    locations.push(loc);
                }
            }
        }

        // Find callers (references), filtered to the definition's file to avoid false positives
        // from unrelated symbols with the same name in other modules.
        let def_file = index
            .find_symbol(&word)
            .await
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|(file, ..)| file);
        if let Some(def_file) = def_file
            && let Ok(callers) = index.find_callers(&word, &def_file).await
        {
            for (file, _caller_name, line, _access) in callers {
                if let Some(loc) = make_location_at_line(&file, &root, line) {
                    locations.push(loc);
                }
            }
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;

        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return Ok(None);
        }

        let line = lines[line_idx];
        let col = position.character as usize;
        let word_info = extract_word_with_range(line, col);

        if word_info.word.is_empty() {
            return Ok(None);
        }

        // Verify this is a known symbol
        let index = self.index.lock().await;
        let index = match index.as_ref() {
            Some(i) => i,
            None => return Ok(None),
        };

        // Check if symbol is known (has a definition in the index)
        if index
            .find_symbol(&word_info.word)
            .await
            .map(|m| m.is_empty())
            .unwrap_or(true)
        {
            return Ok(None);
        }

        Ok(Some(PrepareRenameResponse::Range(Range {
            start: Position {
                line: position.line,
                character: word_info.start_col as u32,
            },
            end: Position {
                line: position.line,
                character: word_info.end_col as u32,
            },
        })))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let file_path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return Ok(None);
        }

        let line = lines[line_idx];
        let col = position.character as usize;
        let old_name = extract_word_with_range(line, col).word;

        if old_name.is_empty() {
            return Ok(None);
        }

        let index = self.index.lock().await;
        let root = self.root.lock().await;

        let (index, root) = match (index.as_ref(), root.as_ref()) {
            (Some(i), Some(r)) => (i, r.clone()),
            _ => return Ok(None),
        };

        // Collect all locations that need renaming
        let mut file_edits: std::collections::HashMap<Url, Vec<TextEdit>> =
            std::collections::HashMap::new();

        // Find definition sites; keep track of def_file to filter callers correctly.
        let mut def_file: Option<String> = None;
        if let Ok(defs) = index.find_symbol(&old_name).await {
            for (file, _kind, start_line, _end_line) in defs {
                if def_file.is_none() {
                    def_file = Some(file.clone());
                }
                let target_path = root.join(&file);
                if let Ok(target_uri) = Url::from_file_path(&target_path)
                    && let Ok(file_content) = std::fs::read_to_string(&target_path)
                    && let Some(edit) =
                        find_rename_edit(&file_content, start_line, &old_name, &new_name)
                {
                    file_edits.entry(target_uri).or_default().push(edit);
                }
            }
        }

        // Find reference sites (callers), filtered to def_file to avoid false positives.
        if let Some(def_file) = &def_file
            && let Ok(callers) = index.find_callers(&old_name, def_file).await
        {
            for (file, _caller_name, line, _access) in callers {
                let target_path = root.join(&file);
                if let Ok(target_uri) = Url::from_file_path(&target_path)
                    && let Ok(file_content) = std::fs::read_to_string(&target_path)
                    && let Some(edit) = find_rename_edit(&file_content, line, &old_name, &new_name)
                {
                    file_edits.entry(target_uri).or_default().push(edit);
                }
            }
        }

        if file_edits.is_empty() {
            return Ok(None);
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(file_edits),
            document_changes: None,
            change_annotations: None,
        }))
    }
}

/// Word at a position with its range.
struct WordAtPosition {
    word: String,
    start_col: usize,
    end_col: usize,
}

/// Extract the word at a given column position in a line, with start/end positions.
/// Build an LSP `Location` pointing to the start of a 1-indexed line number.
fn make_location_at_line(
    file: &str,
    root: &std::path::Path,
    line_1indexed: usize,
) -> Option<Location> {
    let path = root.join(file);
    let uri = Url::from_file_path(&path).ok()?;
    let line = line_1indexed.saturating_sub(1) as u32;
    Some(Location {
        uri,
        range: Range {
            start: Position { line, character: 0 },
            end: Position { line, character: 0 },
        },
    })
}

fn extract_word_with_range(line: &str, col: usize) -> WordAtPosition {
    let chars: Vec<char> = line.chars().collect();
    if col >= chars.len() {
        return WordAtPosition {
            word: String::new(),
            start_col: 0,
            end_col: 0,
        };
    }

    // Find start of word
    let mut start = col;
    while start > 0 && is_identifier_char(chars[start - 1]) {
        start -= 1;
    }

    // Find end of word
    let mut end = col;
    while end < chars.len() && is_identifier_char(chars[end]) {
        end += 1;
    }

    WordAtPosition {
        word: chars[start..end].iter().collect(),
        start_col: start,
        end_col: end,
    }
}

/// Find a rename edit for a symbol at a given line.
fn find_rename_edit(
    content: &str,
    line_num: usize,
    old_name: &str,
    new_name: &str,
) -> Option<TextEdit> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = line_num.saturating_sub(1);
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];

    // Find the symbol in this line (first occurrence)
    // Use word boundary matching to avoid partial matches
    let mut pos = 0;
    while let Some(idx) = line[pos..].find(old_name) {
        let abs_idx = pos + idx;
        let before_ok =
            abs_idx == 0 || !is_identifier_char(line.chars().nth(abs_idx - 1).unwrap_or(' '));
        let after_ok = abs_idx + old_name.len() >= line.len()
            || !is_identifier_char(line.chars().nth(abs_idx + old_name.len()).unwrap_or(' '));

        if before_ok && after_ok {
            return Some(TextEdit {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: abs_idx as u32,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: (abs_idx + old_name.len()) as u32,
                    },
                },
                new_text: new_name.to_string(),
            });
        }
        pos = abs_idx + old_name.len();
    }

    None
}

/// Extract the word at a given column position in a line.
fn extract_word_at_position(line: &str, col: usize) -> String {
    let chars: Vec<char> = line.chars().collect();
    if col >= chars.len() {
        return String::new();
    }

    // Find start of word
    let mut start = col;
    while start > 0 && is_identifier_char(chars[start - 1]) {
        start -= 1;
    }

    // Find end of word
    let mut end = col;
    while end < chars.len() && is_identifier_char(chars[end]) {
        end += 1;
    }

    chars[start..end].iter().collect()
}

/// Check if a character is valid in an identifier.
fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Convert a normalize `Issue` to an LSP `Diagnostic`.
fn issue_to_lsp_diagnostic(issue: &normalize_output::diagnostics::Issue) -> Diagnostic {
    use normalize_output::diagnostics::Severity as S;

    let start_line = issue.line.unwrap_or(1).saturating_sub(1) as u32;
    let start_col = issue.column.unwrap_or(1).saturating_sub(1) as u32;
    let end_line = issue
        .end_line
        .unwrap_or(issue.line.unwrap_or(1))
        .saturating_sub(1) as u32;
    let end_col = issue.end_column.unwrap_or(0) as u32;

    let severity = Some(match issue.severity {
        S::Error => DiagnosticSeverity::ERROR,
        S::Warning => DiagnosticSeverity::WARNING,
        S::Info => DiagnosticSeverity::INFORMATION,
        S::Hint => DiagnosticSeverity::HINT,
    });

    let related_information = if issue.related.is_empty() {
        None
    } else {
        Some(
            issue
                .related
                .iter()
                .filter_map(|rel| {
                    let uri = Url::from_file_path(&rel.file).ok()?;
                    let line = rel.line.unwrap_or(1).saturating_sub(1) as u32;
                    Some(DiagnosticRelatedInformation {
                        location: Location {
                            uri,
                            range: Range {
                                start: Position { line, character: 0 },
                                end: Position { line, character: 0 },
                            },
                        },
                        message: rel.message.clone().unwrap_or_default(),
                    })
                })
                .collect(),
        )
    };

    Diagnostic {
        range: Range {
            start: Position {
                line: start_line,
                character: start_col,
            },
            end: Position {
                line: end_line,
                character: end_col,
            },
        },
        severity,
        code: Some(NumberOrString::String(issue.rule_id.clone())),
        source: Some(format!("normalize/{}", issue.source)),
        message: issue.message.clone(),
        related_information,
        ..Default::default()
    }
}

/// Run rules of a specific type and publish diagnostics to the LSP client.
async fn run_and_publish_diagnostics(
    client: &Client,
    root: &std::path::Path,
    rule_type: &normalize_rules::RuleKind,
    diagnosed_files: &Mutex<HashSet<Url>>,
) {
    let root_owned = root.to_path_buf();
    let rule_type_owned = rule_type.clone();
    let report = tokio::task::spawn_blocking(move || {
        let config = crate::config::NormalizeConfig::load(&root_owned);
        let rules_config = normalize_rules::RulesRunConfig {
            rule_tags: config.rule_tags.0.clone(),
            rules: config.rules.clone(),
        };
        normalize_rules::run_rules_report(
            &root_owned,
            &root_owned,
            None,
            None,
            &rule_type_owned,
            &[],
            &rules_config,
            None,
            &normalize_rules_config::PathFilter::default(),
        )
    })
    .await;

    let report = match report {
        Ok(r) => r,
        Err(e) => {
            client
                .log_message(
                    MessageType::ERROR,
                    format!("Failed to run diagnostics: {e}"),
                )
                .await;
            return;
        }
    };

    // Group issues by file
    let mut by_file: std::collections::HashMap<String, Vec<Diagnostic>> =
        std::collections::HashMap::new();
    for issue in &report.issues {
        by_file
            .entry(issue.file.clone())
            .or_default()
            .push(issue_to_lsp_diagnostic(issue));
    }

    // Clear diagnostics for files that no longer have issues
    let mut prev = diagnosed_files.lock().await;
    let mut new_diagnosed = HashSet::new();

    for (file, diagnostics) in &by_file {
        let file_path = if std::path::Path::new(file).is_absolute() {
            std::path::PathBuf::from(file)
        } else {
            root.join(file)
        };
        if let Ok(uri) = Url::from_file_path(&file_path) {
            client
                .publish_diagnostics(uri.clone(), diagnostics.clone(), None)
                .await;
            new_diagnosed.insert(uri);
        }
    }

    // Clear stale: files in prev but not in new_diagnosed
    for uri in prev.difference(&new_diagnosed) {
        client.publish_diagnostics(uri.clone(), vec![], None).await;
    }

    *prev = new_diagnosed;

    client
        .log_message(
            MessageType::INFO,
            format!(
                "Diagnostics: {} issues in {} files",
                report.issues.len(),
                by_file.len()
            ),
        )
        .await;
}

/// Start the LSP server on stdio.
pub async fn run_lsp_server(root: Option<&std::path::Path>) -> i32 {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(NormalizeBackend::new);

    // If root is provided, initialize early (will be overridden by client's root)
    if let Some(_root) = root {
        // The client will provide the actual root during initialize
    }

    Server::new(stdin, stdout, socket).serve(service).await;
    0
}
