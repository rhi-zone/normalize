//! Shared C/C++ external package resolution.

use crate::external_packages::ResolvedPackage;
use std::path::PathBuf;
use std::process::Command;

/// Get GCC version.
pub fn get_gcc_version() -> Option<String> {
    let output = Command::new("gcc").args(["--version"]).output().ok()?;

    if output.status.success() {
        let version_str = String::from_utf8_lossy(&output.stdout);
        // "gcc (GCC) 13.2.0" or "gcc (Ubuntu 11.4.0-1ubuntu1~22.04) 11.4.0"
        for line in version_str.lines() {
            // Look for version number pattern
            for part in line.split_whitespace() {
                if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    let ver_parts: Vec<&str> = part.split('.').collect();
                    if ver_parts.len() >= 2 {
                        return Some(format!("{}.{}", ver_parts[0], ver_parts[1]));
                    }
                }
            }
        }
    }

    // Try clang as fallback
    let output = Command::new("clang").args(["--version"]).output().ok()?;
    if output.status.success() {
        let version_str = String::from_utf8_lossy(&output.stdout);
        for line in version_str.lines() {
            if line.contains("clang version") {
                for part in line.split_whitespace() {
                    if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                        let ver_parts: Vec<&str> = part.split('.').collect();
                        if ver_parts.len() >= 2 {
                            return Some(format!("{}.{}", ver_parts[0], ver_parts[1]));
                        }
                    }
                }
            }
        }
    }

    None
}

/// Find C/C++ system include directories.
pub fn find_cpp_include_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Standard system include paths
    let system_paths = [
        "/usr/include",
        "/usr/local/include",
        "/usr/include/c++",
        "/usr/include/x86_64-linux-gnu",
        "/usr/include/aarch64-linux-gnu",
    ];

    for path in system_paths {
        let p = PathBuf::from(path);
        if p.is_dir() {
            paths.push(p);
        }
    }

    // Try to get GCC include paths
    if let Ok(output) = Command::new("gcc")
        .args(["-E", "-Wp,-v", "-xc", "/dev/null"])
        .output()
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut in_search_list = false;

        for line in stderr.lines() {
            if line.contains("#include <...> search starts here:") {
                in_search_list = true;
                continue;
            }
            if line.contains("End of search list.") {
                break;
            }
            if in_search_list {
                let path = PathBuf::from(line.trim());
                if path.is_dir() && !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
    }

    // Try clang as well
    if let Ok(output) = Command::new("clang")
        .args(["-E", "-Wp,-v", "-xc", "/dev/null"])
        .output()
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut in_search_list = false;

        for line in stderr.lines() {
            if line.contains("#include <...> search starts here:") {
                in_search_list = true;
                continue;
            }
            if line.contains("End of search list.") {
                break;
            }
            if in_search_list {
                let path = PathBuf::from(line.trim());
                if path.is_dir() && !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
    }

    // macOS specific paths
    #[cfg(target_os = "macos")]
    {
        // Xcode command line tools
        let xcode_paths = [
            "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include",
            "/Library/Developer/CommandLineTools/usr/include",
            "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/usr/include",
        ];
        for path in xcode_paths {
            let p = PathBuf::from(path);
            if p.is_dir() && !paths.contains(&p) {
                paths.push(p);
            }
        }

        // Homebrew
        let homebrew_paths = ["/opt/homebrew/include", "/usr/local/include"];
        for path in homebrew_paths {
            let p = PathBuf::from(path);
            if p.is_dir() && !paths.contains(&p) {
                paths.push(p);
            }
        }
    }

    paths
}

/// Resolve a C/C++ include to a header file.
pub fn resolve_cpp_include(include: &str, include_paths: &[PathBuf]) -> Option<ResolvedPackage> {
    // Strip angle brackets or quotes
    let header = include
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_start_matches('"')
        .trim_end_matches('"');

    // Search through include paths
    for base_path in include_paths {
        let full_path = base_path.join(header);
        if full_path.is_file() {
            return Some(ResolvedPackage {
                path: full_path,
                name: header.to_string(),
                is_namespace: false,
            });
        }

        // For C++ standard library, might be without extension
        if !header.contains('.') {
            // Try with common extensions
            for ext in &["", ".h", ".hpp", ".hxx"] {
                let with_ext = if ext.is_empty() {
                    base_path.join(header)
                } else {
                    base_path.join(format!("{}{}", header, ext))
                };
                if with_ext.is_file() {
                    return Some(ResolvedPackage {
                        path: with_ext,
                        name: header.to_string(),
                        is_namespace: false,
                    });
                }
            }
        }
    }

    None
}
