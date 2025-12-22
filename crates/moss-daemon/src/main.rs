use moss_core::get_moss_dir;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

mod index;
mod symbols;

use index::SymbolIndex;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
enum Request {
    #[serde(rename = "path")]
    Path { query: String },
    #[serde(rename = "symbols")]
    Symbols { file: String },
    #[serde(rename = "callers")]
    Callers { symbol: String },
    #[serde(rename = "callees")]
    Callees { symbol: String, file: String },
    #[serde(rename = "expand")]
    Expand { symbol: String, file: Option<String> },
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "shutdown")]
    Shutdown,
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn ok(data: serde_json::Value) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }
    fn err(msg: &str) -> Self {
        Self { ok: false, data: None, error: Some(msg.to_string()) }
    }
}

struct Daemon {
    root: PathBuf,
    index: Mutex<SymbolIndex>,
}

impl Daemon {
    fn new(root: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let index = SymbolIndex::open(&root)?;
        Ok(Self {
            root,
            index: Mutex::new(index),
        })
    }

    fn handle_request(&self, req: Request) -> Response {
        match req {
            Request::Status => {
                let idx = self.index.lock().unwrap();
                Response::ok(serde_json::json!({
                    "root": self.root.to_string_lossy(),
                    "files": idx.file_count().unwrap_or(0),
                    "symbols": idx.symbol_count().unwrap_or(0),
                }))
            }
            Request::Path { query } => {
                let idx = self.index.lock().unwrap();
                match idx.resolve_path(&query) {
                    Ok(matches) => Response::ok(serde_json::json!(matches)),
                    Err(e) => Response::err(&e.to_string()),
                }
            }
            Request::Symbols { file } => {
                let idx = self.index.lock().unwrap();
                match idx.get_symbols(&file) {
                    Ok(syms) => Response::ok(serde_json::json!(syms)),
                    Err(e) => Response::err(&e.to_string()),
                }
            }
            Request::Callers { symbol } => {
                let idx = self.index.lock().unwrap();
                match idx.find_callers(&symbol) {
                    Ok(callers) => Response::ok(serde_json::json!(callers)),
                    Err(e) => Response::err(&e.to_string()),
                }
            }
            Request::Callees { symbol, file } => {
                let idx = self.index.lock().unwrap();
                match idx.find_callees(&symbol, &file) {
                    Ok(callees) => Response::ok(serde_json::json!(callees)),
                    Err(e) => Response::err(&e.to_string()),
                }
            }
            Request::Expand { symbol, file } => {
                let idx = self.index.lock().unwrap();
                match idx.expand_symbol(&symbol, file.as_deref()) {
                    Ok(source) => Response::ok(serde_json::json!({"source": source})),
                    Err(e) => Response::err(&e.to_string()),
                }
            }
            Request::Shutdown => {
                Response::ok(serde_json::json!({"message": "shutting down"}))
            }
        }
    }

    fn reindex_file(&self, path: &Path) {
        if let Ok(mut idx) = self.index.lock() {
            if let Err(e) = idx.index_file(path) {
                eprintln!("Error indexing {}: {}", path.display(), e);
            }
        }
    }

    fn remove_file(&self, path: &Path) {
        if let Ok(mut idx) = self.index.lock() {
            if let Err(e) = idx.remove_file(path) {
                eprintln!("Error removing {}: {}", path.display(), e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    let moss_dir = get_moss_dir(&root);
    let socket_path = moss_dir.join("daemon.sock");

    // Ensure moss data directory exists
    std::fs::create_dir_all(&moss_dir)?;

    // Remove stale socket
    let _ = std::fs::remove_file(&socket_path);

    let daemon = Arc::new(Daemon::new(root.clone())?);

    // Initial index
    {
        let mut idx = daemon.index.lock().unwrap();
        let count = idx.full_reindex()?;
        eprintln!("Indexed {} files", count);
    }

    // Start file watcher
    let daemon_watcher = daemon.clone();
    let root_watcher = root.clone();
    std::thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();
        watcher.watch(&root_watcher, RecursiveMode::Recursive).unwrap();

        for res in rx {
            match res {
                Ok(event) => {
                    for path in event.paths {
                        // Skip .moss directory
                        if path.to_string_lossy().contains(".moss") {
                            continue;
                        }
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                daemon_watcher.reindex_file(&path);
                            }
                            notify::EventKind::Remove(_) => {
                                daemon_watcher.remove_file(&path);
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => eprintln!("Watch error: {}", e),
            }
        }
    });

    // Start socket server
    let listener = UnixListener::bind(&socket_path)?;
    eprintln!("Daemon listening on {}", socket_path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        let daemon = daemon.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                let response = match serde_json::from_str::<Request>(&line) {
                    Ok(Request::Shutdown) => {
                        let resp = daemon.handle_request(Request::Shutdown);
                        let _ = writer.write_all(serde_json::to_string(&resp).unwrap().as_bytes()).await;
                        let _ = writer.write_all(b"\n").await;
                        std::process::exit(0);
                    }
                    Ok(req) => daemon.handle_request(req),
                    Err(e) => Response::err(&format!("Invalid request: {}", e)),
                };

                let resp_str = serde_json::to_string(&response).unwrap();
                let _ = writer.write_all(resp_str.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
                line.clear();
            }
        });
    }
}
