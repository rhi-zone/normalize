//! Self-update command for normalize CLI.

use std::io::Read;

/// Get the target triple for the current platform
pub fn get_target_triple() -> String {
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    let os = if cfg!(target_os = "linux") {
        "unknown-linux-gnu"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "windows") {
        "pc-windows-msvc"
    } else {
        "unknown"
    };

    format!("{}-{}", arch, os)
}

/// Get the expected asset name for a target
pub fn get_asset_name(target: &str) -> String {
    if target.contains("windows") {
        format!("normalize-{}.zip", target)
    } else {
        format!("normalize-{}.tar.gz", target)
    }
}

/// Extract the normalize binary from a tar.gz archive
pub fn extract_tar_gz(data: &[u8]) -> Result<Vec<u8>, String> {
    let decoder = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;

        if path.file_name().map(|n| n == "normalize").unwrap_or(false) {
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .map_err(|e| e.to_string())?;
            return Ok(contents);
        }
    }

    Err("normalize binary not found in archive".to_string())
}

/// Extract the normalize binary from a zip archive
pub fn extract_zip(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Cursor;

    let reader = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();

        if name == "normalize.exe" || name == "normalize" {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).map_err(|e| e.to_string())?;
            return Ok(contents);
        }
    }

    Err("normalize binary not found in archive".to_string())
}

/// Replace the current binary with new data
pub fn self_replace(new_binary: &[u8]) -> Result<(), String> {
    use std::fs;
    use std::io::Write;

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;

    // Create temp file in same directory (for atomic rename on same filesystem)
    let temp_path = current_exe.with_extension("new");
    let backup_path = current_exe.with_extension("old");

    // Write new binary to temp file
    let mut temp_file = fs::File::create(&temp_path).map_err(|e| e.to_string())?;
    temp_file.write_all(new_binary).map_err(|e| e.to_string())?;
    temp_file.sync_all().map_err(|e| e.to_string())?;
    drop(temp_file);

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&temp_path, perms).map_err(|e| e.to_string())?;
    }

    // Rename current to backup
    if backup_path.exists() {
        fs::remove_file(&backup_path).ok();
    }
    fs::rename(&current_exe, &backup_path).map_err(|e| format!("backup failed: {}", e))?;

    // Rename new to current
    if let Err(e) = fs::rename(&temp_path, &current_exe) {
        // Try to restore backup
        let _ = fs::rename(&backup_path, &current_exe);
        return Err(format!("install failed: {}", e));
    }

    // Remove backup
    fs::remove_file(&backup_path).ok();

    Ok(())
}

/// Compute the SHA256 hex digest of data.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Simple version comparison (semver-like)
pub fn version_gt(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.split('-').next()?.parse().ok())
            .collect()
    };

    let va = parse(a);
    let vb = parse(b);

    for (a, b) in va.iter().zip(vb.iter()) {
        match a.cmp(b) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => continue,
        }
    }
    va.len() > vb.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_empty() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hex_abc() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
