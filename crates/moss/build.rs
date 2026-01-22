//! Build script for moss - rebuilds SPA when source changes.

#[cfg(feature = "sessions-web")]
use std::fs;
#[cfg(feature = "sessions-web")]
use std::io::Write;
#[cfg(feature = "sessions-web")]
use std::path::Path;
#[cfg(feature = "sessions-web")]
use std::process::Command;
#[cfg(feature = "sessions-web")]
use std::time::SystemTime;

#[cfg(feature = "sessions-web")]
use flate2::Compression;
#[cfg(feature = "sessions-web")]
use flate2::write::GzEncoder;

fn main() {
    // Only build SPA when sessions-web feature is enabled
    // Library consumers don't need it, CI/release builds enable it
    #[cfg(feature = "sessions-web")]
    rebuild_spa_if_needed();
}

#[cfg(feature = "sessions-web")]
fn rebuild_spa_if_needed() {
    let web_dir = Path::new("../../web/sessions");
    let src_dir = web_dir.join("src");
    let dist_dir = web_dir.join("dist");

    // Tell cargo to rerun if source or dist files change/disappear
    println!("cargo:rerun-if-changed=../../web/sessions/src");
    println!("cargo:rerun-if-changed=../../web/sessions/index.html");
    println!("cargo:rerun-if-changed=../../web/sessions/package.json");
    println!("cargo:rerun-if-changed=../../web/sessions/dist/index.html.gz");
    println!("cargo:rerun-if-changed=../../web/sessions/dist/app.js.gz");
    println!("cargo:rerun-if-changed=../../web/sessions/dist/index.css.gz");

    if !src_dir.exists() {
        return; // No web source, skip
    }

    // Get newest source mtime
    let src_mtime = newest_mtime(&src_dir).or_else(|| file_mtime(&web_dir.join("index.html")));

    // Get oldest dist mtime (if any dist file is older than newest source, rebuild)
    let dist_mtime = if dist_dir.exists() {
        oldest_mtime(&dist_dir)
    } else {
        None
    };

    let needs_rebuild = match (src_mtime, dist_mtime) {
        (Some(src), Some(dist)) => src > dist,
        (Some(_), None) => true, // No dist, need to build
        _ => false,
    };

    if !needs_rebuild {
        return;
    }

    eprintln!("Rebuilding sessions SPA...");

    // Try bun first, fall back to npm
    let result = Command::new("bun")
        .args(["run", "build"])
        .current_dir(web_dir)
        .status()
        .or_else(|_| {
            Command::new("npm")
                .args(["run", "build"])
                .current_dir(web_dir)
                .status()
        });

    match result {
        Ok(status) if status.success() => {
            // Gzip the dist files for smaller binary embedding
            gzip_file(&dist_dir.join("index.html"));
            gzip_file(&dist_dir.join("app.js"));
            gzip_file(&dist_dir.join("index.css"));
            eprintln!("SPA rebuild complete (gzipped)");
        }
        Ok(status) => {
            panic!(
                "SPA build failed with status {}. Run manually: cd web/sessions && bun run build",
                status
            );
        }
        Err(e) => {
            panic!(
                "Could not build SPA (bun/npm not found: {}). Install bun or run: cd web/sessions && bun run build",
                e
            );
        }
    }
}

#[cfg(feature = "sessions-web")]
fn gzip_file(path: &Path) {
    let content = fs::read(path).expect("Failed to read file for gzip");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&content).expect("Failed to gzip");
    let compressed = encoder.finish().expect("Failed to finish gzip");
    let gz_path = path.with_extension(format!(
        "{}.gz",
        path.extension().unwrap_or_default().to_str().unwrap_or("")
    ));
    fs::write(&gz_path, compressed).expect("Failed to write gzipped file");
}

#[cfg(feature = "sessions-web")]
fn newest_mtime(dir: &Path) -> Option<SystemTime> {
    walkdir(dir).filter_map(|p| file_mtime(&p)).max()
}

#[cfg(feature = "sessions-web")]
fn oldest_mtime(dir: &Path) -> Option<SystemTime> {
    walkdir(dir).filter_map(|p| file_mtime(&p)).min()
}

#[cfg(feature = "sessions-web")]
fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

#[cfg(feature = "sessions-web")]
fn walkdir(dir: &Path) -> impl Iterator<Item = std::path::PathBuf> {
    walkdir_impl(dir).into_iter()
}

#[cfg(feature = "sessions-web")]
fn walkdir_impl(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir_impl(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}
