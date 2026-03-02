//! Package registry queries.

use normalize_ecosystems::{
    AuditResult, PackageError, PackageInfo, VulnerabilitySeverity, all_ecosystems,
    detect_all_ecosystems,
};
use nu_ansi_term::Color::Yellow;
use std::path::Path;

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub enum PackageAction {
    /// Query package info from registry
    Info {
        /// Package name to query (optionally with @version)
        package: String,
    },
    /// List declared dependencies from manifest
    List,
    /// Show dependency tree from lockfile
    Tree,
    /// Show why a dependency is in the tree
    Why {
        /// Package name to trace
        package: String,
    },
    /// Show outdated packages (installed vs latest)
    Outdated,
    /// Check for security vulnerabilities
    Audit,
}

pub fn cmd_package(action: PackageAction, ecosystem: Option<&str>, root: Option<&Path>) -> i32 {
    let project_root = root.unwrap_or(Path::new("."));
    let use_colors = false;

    // Get ecosystem either by name or by detection
    if let Some(name) = ecosystem {
        // Explicit ecosystem specified
        match find_ecosystem_by_name(name) {
            Some(eco) => run_for_ecosystem(eco, &action, project_root, use_colors),
            None => {
                eprintln!("error: unknown ecosystem '{}'", name);
                eprintln!("available: {}", available_ecosystems().join(", "));
                1
            }
        }
    } else {
        // Auto-detect ecosystems
        let ecosystems = detect_all_ecosystems(project_root);
        if ecosystems.is_empty() {
            eprintln!("error: could not detect ecosystem from project files");
            eprintln!("hint: use --ecosystem to specify explicitly");
            eprintln!("available: {}", available_ecosystems().join(", "));
            return 1;
        }

        // For list/tree, run for all detected ecosystems
        // For info/outdated, use first ecosystem only
        match &action {
            PackageAction::List | PackageAction::Tree => {
                let mut exit_code = 0;
                for (i, eco) in ecosystems.iter().enumerate() {
                    if i > 0 {
                        println!(); // Separator between ecosystems
                    }
                    let result = run_for_ecosystem(*eco, &action, project_root, use_colors);
                    if result != 0 {
                        exit_code = result;
                    }
                }
                exit_code
            }
            _ => {
                if ecosystems.len() > 1 {
                    let names: Vec<_> = ecosystems.iter().map(|e| e.name()).collect();
                    eprintln!("note: multiple ecosystems detected: {}", names.join(", "));
                    eprintln!("hint: use --ecosystem to specify which one");
                }
                run_for_ecosystem(ecosystems[0], &action, project_root, use_colors)
            }
        }
    }
}

fn run_for_ecosystem(
    eco: &dyn normalize_ecosystems::Ecosystem,
    action: &PackageAction,
    project_root: &Path,
    use_colors: bool,
) -> i32 {
    match action {
        PackageAction::Info { package } => cmd_info(eco, package, project_root),
        PackageAction::List => cmd_list(eco, project_root, use_colors),
        PackageAction::Tree => cmd_tree(eco, project_root, use_colors),
        PackageAction::Why { package } => cmd_why(eco, package, project_root, use_colors),
        PackageAction::Outdated => cmd_outdated(eco, project_root, use_colors),
        PackageAction::Audit => cmd_audit(eco, project_root),
    }
}

fn cmd_info(eco: &dyn normalize_ecosystems::Ecosystem, package: &str, project_root: &Path) -> i32 {
    match eco.query(package, project_root) {
        Ok(info) => {
            print_human(&info, eco.name());
            0
        }
        Err(e) => {
            match e {
                PackageError::NotFound(name) => {
                    eprintln!(
                        "error: package '{}' not found in {} registry",
                        name,
                        eco.name()
                    );
                }
                PackageError::NoToolFound => {
                    eprintln!("error: no {} tools found in PATH", eco.name());
                    eprintln!("hint: install one of: {:?}", eco.tools());
                }
                _ => {
                    eprintln!("error: {}", e);
                }
            }
            1
        }
    }
}

fn cmd_list(
    eco: &dyn normalize_ecosystems::Ecosystem,
    project_root: &Path,
    use_colors: bool,
) -> i32 {
    match eco.list_dependencies(project_root) {
        Ok(deps) => {
            println!("{} dependencies ({})", deps.len(), eco.name());
            println!();
            for dep in &deps {
                let version = dep.version_req.as_deref().unwrap_or("*");
                let version_display = if use_colors {
                    Yellow.paint(version).to_string()
                } else {
                    version.to_string()
                };
                let optional = if dep.optional { " (optional)" } else { "" };
                println!("  {} {}{}", dep.name, version_display, optional);
            }
            0
        }
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

fn cmd_tree(
    eco: &dyn normalize_ecosystems::Ecosystem,
    project_root: &Path,
    use_colors: bool,
) -> i32 {
    match eco.dependency_tree(project_root) {
        Ok(tree) => {
            print_tree(&tree, use_colors);
            0
        }
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

fn print_tree(tree: &normalize_ecosystems::DependencyTree, use_colors: bool) {
    for root in &tree.roots {
        print_node(root, 0, use_colors);
    }
}

fn print_node(node: &normalize_ecosystems::TreeNode, depth: usize, use_colors: bool) {
    let indent = "  ".repeat(depth);
    if node.version.is_empty() {
        println!("{}{}", indent, node.name);
    } else {
        let version_display = if use_colors {
            Yellow.paint(format!("v{}", node.version)).to_string()
        } else {
            format!("v{}", node.version)
        };
        println!("{}{} {}", indent, node.name, version_display);
    }
    for child in &node.dependencies {
        print_node(child, depth + 1, use_colors);
    }
}

fn cmd_why(
    eco: &dyn normalize_ecosystems::Ecosystem,
    package: &str,
    project_root: &Path,
    use_colors: bool,
) -> i32 {
    match eco.dependency_tree(project_root) {
        Ok(tree) => {
            let paths = find_dependency_paths(&tree, package);

            if paths.is_empty() {
                println!("Package '{}' not found in dependency tree", package);
                return 1;
            }

            println!("'{}' is required by {} path(s):", package, paths.len());
            println!();
            for (i, path) in paths.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                for (j, (name, version)) in path.iter().enumerate() {
                    let indent = "  ".repeat(j);
                    if version.is_empty() {
                        println!("{}{}", indent, name);
                    } else {
                        let version_display = if use_colors {
                            Yellow.paint(format!("v{}", version)).to_string()
                        } else {
                            format!("v{}", version)
                        };
                        println!("{}{} {}", indent, name, version_display);
                    }
                }
            }
            0
        }
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

/// Find all paths from root packages to the target dependency.
fn find_dependency_paths(
    tree: &normalize_ecosystems::DependencyTree,
    target: &str,
) -> Vec<Vec<(String, String)>> {
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
    // Check if current node is the target
    if node.name == target || node.name.ends_with(&format!("/{}", target)) {
        all_paths.push(current_path.clone());
        return;
    }

    // Recurse into dependencies
    for child in &node.dependencies {
        current_path.push((child.name.clone(), child.version.clone()));
        find_paths_recursive(child, target, current_path, all_paths);
        current_path.pop();
    }
}

fn cmd_outdated(
    eco: &dyn normalize_ecosystems::Ecosystem,
    project_root: &Path,
    use_colors: bool,
) -> i32 {
    // Get declared dependencies
    let deps = match eco.list_dependencies(project_root) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    #[derive(serde::Serialize)]
    struct OutdatedPackage {
        name: String,
        installed: Option<String>,
        latest: String,
        wanted: Option<String>,
    }

    let mut outdated = Vec::new();
    let mut errors = Vec::new();

    for dep in &deps {
        // Get installed version from lockfile
        let installed = eco.installed_version(&dep.name, project_root);

        // Get latest version from registry
        match eco.query(&dep.name, project_root) {
            Ok(info) => {
                // Only show if installed differs from latest
                let is_outdated = match &installed {
                    Some(v) => v != &info.version,
                    None => true, // Not installed = show it
                };

                if is_outdated {
                    outdated.push(OutdatedPackage {
                        name: dep.name.clone(),
                        installed: installed.clone(),
                        latest: info.version,
                        wanted: dep.version_req.clone(),
                    });
                }
            }
            Err(e) => {
                errors.push((dep.name.clone(), e.to_string()));
            }
        }
    }

    if outdated.is_empty() && errors.is_empty() {
        println!("All packages are up to date");
    } else {
        if !outdated.is_empty() {
            println!("Outdated packages ({}):", outdated.len());
            println!();
            for pkg in &outdated {
                let installed = pkg.installed.as_deref().unwrap_or("(not installed)");
                let (installed_display, latest_display) = if use_colors {
                    (
                        Yellow.paint(installed).to_string(),
                        Yellow.paint(&pkg.latest).to_string(),
                    )
                } else {
                    (installed.to_string(), pkg.latest.clone())
                };
                println!("  {} {} → {}", pkg.name, installed_display, latest_display);
            }
        }
        if !errors.is_empty() {
            println!();
            println!("Errors ({}):", errors.len());
            for (name, err) in &errors {
                println!("  {}: {}", name, err);
            }
        }
    }

    0
}

fn cmd_audit(eco: &dyn normalize_ecosystems::Ecosystem, project_root: &Path) -> i32 {
    match eco.audit(project_root) {
        Ok(result) => {
            print_audit_human(&result, eco.name());
            if result.vulnerabilities.iter().any(|v| {
                matches!(
                    v.severity,
                    VulnerabilitySeverity::Critical | VulnerabilitySeverity::High
                )
            }) {
                1 // Exit with error if high/critical vulnerabilities found
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

fn print_audit_human(result: &AuditResult, ecosystem: &str) {
    if result.vulnerabilities.is_empty() {
        println!("No vulnerabilities found ({}).", ecosystem);
        return;
    }

    let critical = result
        .vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::Critical)
        .count();
    let high = result
        .vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::High)
        .count();
    let medium = result
        .vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::Medium)
        .count();
    let low = result
        .vulnerabilities
        .iter()
        .filter(|v| v.severity == VulnerabilitySeverity::Low)
        .count();

    println!(
        "Found {} vulnerabilities ({}) - {} critical, {} high, {} medium, {} low",
        result.vulnerabilities.len(),
        ecosystem,
        critical,
        high,
        medium,
        low
    );
    println!();

    for vuln in &result.vulnerabilities {
        let severity = vuln.severity.as_str();
        println!(
            "[{}] {} {} - {}",
            severity.to_uppercase(),
            vuln.package,
            vuln.version,
            vuln.title
        );

        if let Some(cve) = &vuln.cve {
            println!("  CVE: {}", cve);
        }
        if let Some(url) = &vuln.url {
            println!("  URL: {}", url);
        }
        if let Some(fixed) = &vuln.fixed_in {
            println!("  Fixed in: {}", fixed);
        }
        println!();
    }
}

fn find_ecosystem_by_name(name: &str) -> Option<&'static dyn normalize_ecosystems::Ecosystem> {
    all_ecosystems().iter().find(|e| e.name() == name).copied()
}

fn available_ecosystems() -> Vec<&'static str> {
    all_ecosystems().iter().map(|e| e.name()).collect()
}

/// Service-callable package command.
/// Dispatches to the appropriate subcommand based on `action` string.
#[cfg(feature = "cli")]
pub fn cmd_package_service(
    action: &str,
    package: Option<&str>,
    ecosystem: Option<&str>,
    root: Option<&str>,
    _use_colors: bool,
) -> Result<crate::service::package::PackageResult, String> {
    let package_action = match action {
        "info" => {
            let pkg = package.ok_or("package name required")?;
            PackageAction::Info {
                package: pkg.to_string(),
            }
        }
        "list" => PackageAction::List,
        "tree" => PackageAction::Tree,
        "why" => {
            let pkg = package.ok_or("package name required")?;
            PackageAction::Why {
                package: pkg.to_string(),
            }
        }
        "outdated" => PackageAction::Outdated,
        "audit" => PackageAction::Audit,
        _ => return Err(format!("unknown package action: {}", action)),
    };

    let root_path = root.map(std::path::Path::new);
    let exit_code = cmd_package(package_action, ecosystem, root_path);
    if exit_code == 0 {
        Ok(crate::service::package::PackageResult {
            success: true,
            message: None,
            data: None,
        })
    } else {
        Err("Command failed".to_string())
    }
}

fn print_human(info: &PackageInfo, ecosystem: &str) {
    println!("{} {} ({})", info.name, info.version, ecosystem);

    if let Some(desc) = &info.description {
        println!("{}", desc);
    }

    println!();

    if let Some(license) = &info.license {
        println!("license: {}", license);
    }

    if let Some(homepage) = &info.homepage {
        println!("homepage: {}", homepage);
    }

    if let Some(repo) = &info.repository {
        println!("repository: {}", repo);
    }

    if !info.features.is_empty() {
        println!();
        println!("features:");
        for feature in &info.features {
            if feature.dependencies.is_empty() {
                println!("  {}", feature.name);
            } else {
                println!("  {} = [{}]", feature.name, feature.dependencies.join(", "));
            }
        }
    }

    if !info.dependencies.is_empty() {
        println!();
        println!("dependencies:");
        for dep in &info.dependencies {
            let version = dep.version_req.as_deref().unwrap_or("*");
            let optional = if dep.optional { " (optional)" } else { "" };
            println!("  {} {}{}", dep.name, version, optional);
        }
    }
}
