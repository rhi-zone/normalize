//! Package registry queries.

use normalize_ecosystems::{
    Dependency, DependencyTree, PackageError, PackageInfo, Vulnerability, VulnerabilitySeverity,
    all_ecosystems, detect_all_ecosystems,
};
use std::path::Path;

// ── Public data-fetching functions used by the service layer ─────────────────

/// Get package info for a single package, returning (ecosystem_name, PackageInfo).
pub fn get_info(
    package: &str,
    ecosystem: Option<&str>,
    root: &Path,
) -> Result<(String, PackageInfo), String> {
    let eco = resolve_single_ecosystem(ecosystem, root)?;
    match eco.query(package, root) {
        Ok(info) => Ok((eco.name().to_string(), info)),
        Err(e) => Err(format_package_error(&e, eco.name())),
    }
}

/// Get declared dependencies, returning (ecosystem_name, Vec<Dependency>).
pub fn get_list(ecosystem: Option<&str>, root: &Path) -> Result<(String, Vec<Dependency>), String> {
    let eco = resolve_single_ecosystem(ecosystem, root)?;
    match eco.list_dependencies(root) {
        Ok(deps) => Ok((eco.name().to_string(), deps)),
        Err(e) => Err(format!("error: {}", e)),
    }
}

/// Get the dependency tree, returning (ecosystem_name, DependencyTree).
pub fn get_tree(ecosystem: Option<&str>, root: &Path) -> Result<(String, DependencyTree), String> {
    let eco = resolve_single_ecosystem(ecosystem, root)?;
    match eco.dependency_tree(root) {
        Ok(tree) => Ok((eco.name().to_string(), tree)),
        Err(e) => Err(format!("error: {}", e)),
    }
}

/// A single outdated package entry: (name, installed_version, latest_version, wanted_constraint).
pub type OutdatedRow = (String, Option<String>, String, Option<String>);

/// Result type for `show_outdated_data`.
pub type OutdatedResult = Result<(String, Vec<OutdatedRow>, Vec<(String, String)>), String>;

/// Get outdated packages, returning (ecosystem_name, outdated_entries, errors).
pub fn show_outdated_data(ecosystem: Option<&str>, root: &Path) -> OutdatedResult {
    let eco = resolve_single_ecosystem(ecosystem, root)?;
    let deps = match eco.list_dependencies(root) {
        Ok(d) => d,
        Err(e) => return Err(format!("error: {}", e)),
    };

    let mut outdated = Vec::new();
    let mut errors = Vec::new();

    for dep in &deps {
        let installed = eco.installed_version(&dep.name, root);
        match eco.query(&dep.name, root) {
            Ok(info) => {
                let is_outdated = match &installed {
                    Some(v) => v != &info.version,
                    None => true,
                };
                if is_outdated {
                    outdated.push((
                        dep.name.clone(),
                        installed,
                        info.version,
                        dep.version_req.clone(),
                    ));
                }
            }
            Err(e) => {
                errors.push((dep.name.clone(), e.to_string()));
            }
        }
    }

    Ok((eco.name().to_string(), outdated, errors))
}

/// Get audit results, returning (ecosystem_name, Vec<Vulnerability>).
pub fn get_audit(
    ecosystem: Option<&str>,
    root: &Path,
) -> Result<(String, Vec<Vulnerability>), String> {
    let eco = resolve_single_ecosystem(ecosystem, root)?;
    match eco.audit(root) {
        Ok(result) => Ok((eco.name().to_string(), result.vulnerabilities)),
        Err(e) => Err(format!("error: {}", e)),
    }
}

// ── Formatting helpers used by report format_text() impls ────────────────────

/// Format a package info response in human-readable form.
pub fn print_human(info: &PackageInfo, ecosystem: &str) -> String {
    let mut out = format!("{} {} ({})", info.name, info.version, ecosystem);

    if let Some(desc) = &info.description {
        out.push('\n');
        out.push_str(desc);
    }

    out.push('\n');

    if let Some(license) = &info.license {
        out.push('\n');
        out.push_str(&format!("license: {}", license));
    }

    if let Some(homepage) = &info.homepage {
        out.push('\n');
        out.push_str(&format!("homepage: {}", homepage));
    }

    if let Some(repo) = &info.repository {
        out.push('\n');
        out.push_str(&format!("repository: {}", repo));
    }

    if !info.features.is_empty() {
        out.push_str("\n\nfeatures:");
        for feature in &info.features {
            if feature.dependencies.is_empty() {
                out.push_str(&format!("\n  {}", feature.name));
            } else {
                out.push_str(&format!(
                    "\n  {} = [{}]",
                    feature.name,
                    feature.dependencies.join(", ")
                ));
            }
        }
    }

    if !info.dependencies.is_empty() {
        out.push_str("\n\ndependencies:");
        for dep in &info.dependencies {
            let version = dep.version_req.as_deref().unwrap_or("*");
            let optional = if dep.optional { " (optional)" } else { "" };
            out.push_str(&format!("\n  {} {}{}", dep.name, version, optional));
        }
    }

    out
}

/// Format a dependency tree in human-readable form.
pub fn print_tree(tree: &DependencyTree) -> String {
    let mut out = String::new();
    for root in &tree.roots {
        format_node_into(&mut out, root, 0);
    }
    out.trim_end().to_string()
}

fn format_node_into(out: &mut String, node: &normalize_ecosystems::TreeNode, depth: usize) {
    let indent = "  ".repeat(depth);
    if node.version.is_empty() {
        out.push_str(&format!("{}{}\n", indent, node.name));
    } else {
        out.push_str(&format!("{}{} v{}\n", indent, node.name, node.version));
    }
    for child in &node.dependencies {
        format_node_into(out, child, depth + 1);
    }
}

/// Format an audit result in human-readable form.
pub fn print_audit_human(vulnerabilities: &[Vulnerability], ecosystem: &str) -> String {
    if vulnerabilities.is_empty() {
        return format!("No vulnerabilities found ({}).", ecosystem);
    }

    let critical = vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::Critical)
        .count();
    let high = vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::High)
        .count();
    let medium = vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::Medium)
        .count();
    let low = vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::Low)
        .count();

    let mut out = format!(
        "Found {} vulnerabilities ({}) - {} critical, {} high, {} medium, {} low\n",
        vulnerabilities.len(),
        ecosystem,
        critical,
        high,
        medium,
        low
    );

    for vuln in vulnerabilities {
        let severity = vuln.severity.as_str();
        out.push('\n');
        out.push_str(&format!(
            "[{}] {} {} - {}",
            severity.to_uppercase(),
            vuln.package,
            vuln.version,
            vuln.title
        ));

        if let Some(cve) = &vuln.cve {
            out.push_str(&format!("\n  CVE: {}", cve));
        }
        if let Some(url) = &vuln.url {
            out.push_str(&format!("\n  URL: {}", url));
        }
        if let Some(fixed) = &vuln.fixed_in {
            out.push_str(&format!("\n  Fixed in: {}", fixed));
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

// ── Path-finding helpers (used by service layer) ─────────────────────────────

/// Find all paths from root packages to the target dependency.
pub fn find_dependency_paths(tree: &DependencyTree, target: &str) -> Vec<Vec<(String, String)>> {
    let mut all_paths = Vec::new();

    for root in &tree.roots {
        let mut current_path = vec![(root.name.clone(), root.version.clone())];
        find_paths_recursive(root, target, &mut current_path, &mut all_paths);
    }

    all_paths
}

fn find_paths_recursive(
    node: &normalize_ecosystems::TreeNode,
    target: &str,
    current_path: &mut Vec<(String, String)>,
    all_paths: &mut Vec<Vec<(String, String)>>,
) {
    if node.name == target || node.name.ends_with(&format!("/{}", target)) {
        all_paths.push(current_path.clone());
        return;
    }

    for child in &node.dependencies {
        current_path.push((child.name.clone(), child.version.clone()));
        find_paths_recursive(child, target, current_path, all_paths);
        current_path.pop();
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn resolve_single_ecosystem(
    ecosystem: Option<&str>,
    root: &Path,
) -> Result<&'static dyn normalize_ecosystems::Ecosystem, String> {
    if let Some(name) = ecosystem {
        find_ecosystem_by_name(name).ok_or_else(|| {
            format!(
                "error: unknown ecosystem '{}'\navailable: {}",
                name,
                available_ecosystems().join(", ")
            )
        })
    } else {
        let ecosystems = detect_all_ecosystems(root);
        if ecosystems.is_empty() {
            return Err(format!(
                "error: could not detect ecosystem from project files\nhint: use --ecosystem to specify explicitly\navailable: {}",
                available_ecosystems().join(", ")
            ));
        }
        if ecosystems.len() > 1 {
            let names: Vec<_> = ecosystems.iter().map(|e| e.name()).collect();
            eprintln!("note: multiple ecosystems detected: {}", names.join(", "));
            eprintln!("hint: use --ecosystem to specify which one");
        }
        Ok(ecosystems[0])
    }
}

fn format_package_error(e: &PackageError, ecosystem: &str) -> String {
    match e {
        PackageError::NotFound(name) => {
            format!(
                "error: package '{}' not found in {} registry",
                name, ecosystem
            )
        }
        PackageError::NoToolFound => {
            format!(
                "error: no {} tools found in PATH\nhint: install one of the required tools",
                ecosystem
            )
        }
        _ => format!("error: {}", e),
    }
}

fn find_ecosystem_by_name(name: &str) -> Option<&'static dyn normalize_ecosystems::Ecosystem> {
    all_ecosystems().iter().find(|e| e.name() == name).copied()
}

fn available_ecosystems() -> Vec<&'static str> {
    all_ecosystems().iter().map(|e| e.name()).collect()
}
