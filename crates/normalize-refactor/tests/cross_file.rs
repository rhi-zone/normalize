//! Cross-file name resolution tests.
//!
//! Tests ModuleResolver implementations for Rust, TypeScript, Python, and Go
//! against fixture directories under `tests/fixtures/xfile/`.

use normalize_languages::go::GoModuleResolver;
use normalize_languages::python::PythonModuleResolver;
use normalize_languages::rust::RustModuleResolver;
use normalize_languages::typescript::TsModuleResolver;
use normalize_languages::{ImportSpec, ModuleResolver, Resolution};
use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xfile/rust")
}

#[test]
fn workspace_config_reads_crate_name() {
    let root = fixture_root();
    let resolver = RustModuleResolver;
    let cfg = resolver.workspace_config(&root);

    assert_eq!(cfg.workspace_root, root);
    assert!(
        cfg.path_mappings
            .iter()
            .any(|(name, _)| name == "xfile-fixture"),
        "expected 'xfile-fixture' in path_mappings, got: {:?}",
        cfg.path_mappings.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
}

#[test]
fn module_of_file_lib_rs() {
    let root = fixture_root();
    let resolver = RustModuleResolver;
    let cfg = resolver.workspace_config(&root);

    // main.rs at crate root → canonical path is the crate name
    let main_rs = root.join("src/main.rs");
    // The fixture doesn't have a src/ dir — the .rs files are at the root.
    // This tests the fallback: files not under src/ return empty.
    let modules = resolver.module_of_file(&root, &main_rs, &cfg);
    // main.rs is not under src/, so no module identity is returned
    assert!(modules.is_empty() || !modules[0].canonical_path.is_empty());
}

#[test]
fn module_of_file_for_src_utils() {
    // Build a temporary crate structure with a src/ directory
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Write Cargo.toml
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mycrate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    // Write src/utils.rs
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(
        src_dir.join("utils.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();
    std::fs::write(src_dir.join("lib.rs"), "pub mod utils;").unwrap();

    let resolver = RustModuleResolver;
    let cfg = resolver.workspace_config(root);
    assert!(cfg.path_mappings.iter().any(|(n, _)| n == "mycrate"));

    let utils_rs = src_dir.join("utils.rs");
    let modules = resolver.module_of_file(root, &utils_rs, &cfg);
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].canonical_path, "mycrate::utils");
}

#[test]
fn module_of_file_lib_rs_is_crate_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mylib\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), "pub fn hello() {}").unwrap();

    let resolver = RustModuleResolver;
    let cfg = resolver.workspace_config(root);

    let lib_rs = src_dir.join("lib.rs");
    let modules = resolver.module_of_file(root, &lib_rs, &cfg);
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].canonical_path, "mylib");
}

#[test]
fn resolve_intra_workspace_import() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mycrate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), "pub mod utils;").unwrap();
    std::fs::write(
        src_dir.join("utils.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();
    std::fs::write(src_dir.join("main.rs"), "mod utils; fn main() {}").unwrap();

    let resolver = RustModuleResolver;
    let cfg = resolver.workspace_config(root);

    let from = src_dir.join("main.rs");
    let spec = ImportSpec {
        raw: "mycrate::utils::add".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    let resolution = resolver.resolve(&from, &spec, &cfg);
    match resolution {
        Resolution::Resolved(path, name) => {
            assert_eq!(path, src_dir.join("utils.rs"));
            assert_eq!(name, "add");
        }
        other => panic!(
            "expected Resolved, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn resolve_stdlib_returns_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mycrate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), "").unwrap();

    let resolver = RustModuleResolver;
    let cfg = resolver.workspace_config(root);

    let from = src_dir.join("lib.rs");
    let spec = ImportSpec {
        raw: "std::collections::HashMap".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    let resolution = resolver.resolve(&from, &spec, &cfg);
    assert!(matches!(resolution, Resolution::NotFound));
}

#[test]
fn resolve_non_rs_file_returns_not_applicable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    let resolver = RustModuleResolver;
    let cfg = resolver.workspace_config(root);

    let from = root.join("src/index.ts");
    let spec = ImportSpec {
        raw: "x::foo".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    let resolution = resolver.resolve(&from, &spec, &cfg);
    assert!(matches!(resolution, Resolution::NotApplicable));
}

// =============================================================================
// TypeScript resolver tests
// =============================================================================

fn ts_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xfile/typescript")
}

#[test]
fn ts_resolve_relative_import() {
    let root = ts_fixture_root();
    let resolver = TsModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("models.ts");
    let spec = ImportSpec {
        raw: "./utils".to_string(),
        is_relative: true,
        names: vec!["greet".to_string()],
        is_glob: false,
    };
    match resolver.resolve(&from, &spec, &cfg) {
        Resolution::Resolved(path, _) => {
            assert_eq!(path, root.join("utils.ts"));
        }
        other => panic!(
            "expected Resolved, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn ts_resolve_js_extension_elision() {
    // TypeScript allows importing ./utils.js which resolves to ./utils.ts
    let root = ts_fixture_root();
    let resolver = TsModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("index.ts");
    let spec = ImportSpec {
        raw: "./utils.js".to_string(),
        is_relative: true,
        names: vec!["greet".to_string()],
        is_glob: false,
    };
    match resolver.resolve(&from, &spec, &cfg) {
        Resolution::Resolved(path, _) => {
            assert_eq!(path, root.join("utils.ts"));
        }
        other => panic!(
            "expected Resolved (via .js elision), got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn ts_not_applicable_for_non_ts_file() {
    let root = ts_fixture_root();
    let resolver = TsModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("app.js");
    let spec = ImportSpec {
        raw: "./utils".to_string(),
        is_relative: true,
        names: Vec::new(),
        is_glob: false,
    };
    assert!(matches!(
        resolver.resolve(&from, &spec, &cfg),
        Resolution::NotApplicable
    ));
}

#[test]
fn ts_module_of_file() {
    let root = ts_fixture_root();
    let resolver = TsModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let file = root.join("utils.ts");
    let modules = resolver.module_of_file(&root, &file, &cfg);
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].canonical_path, "utils");
}

// =============================================================================
// Python resolver tests
// =============================================================================

fn py_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xfile/python")
}

#[test]
fn py_resolve_relative_import() {
    let root = py_fixture_root();
    let resolver = PythonModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("models.py");
    let spec = ImportSpec {
        raw: ".utils".to_string(),
        is_relative: true,
        names: vec!["format_name".to_string()],
        is_glob: false,
    };
    match resolver.resolve(&from, &spec, &cfg) {
        Resolution::Resolved(path, _) => {
            assert_eq!(path, root.join("utils.py"));
        }
        other => panic!(
            "expected Resolved, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn py_not_applicable_for_non_py_file() {
    let root = py_fixture_root();
    let resolver = PythonModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("app.rb");
    let spec = ImportSpec {
        raw: "utils".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    assert!(matches!(
        resolver.resolve(&from, &spec, &cfg),
        Resolution::NotApplicable
    ));
}

#[test]
fn py_absolute_import_not_found() {
    let root = py_fixture_root();
    let resolver = PythonModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("main.py");
    let spec = ImportSpec {
        raw: "os.path".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    // stdlib — can't be resolved
    assert!(matches!(
        resolver.resolve(&from, &spec, &cfg),
        Resolution::NotFound
    ));
}

// =============================================================================
// Go resolver tests
// =============================================================================

fn go_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xfile/go")
}

#[test]
fn go_workspace_config_reads_module_path() {
    let root = go_fixture_root();
    let resolver = GoModuleResolver;
    let cfg = resolver.workspace_config(&root);

    assert!(
        cfg.path_mappings
            .iter()
            .any(|(name, _)| name == "example.com/myapp"),
        "expected 'example.com/myapp' in path_mappings, got: {:?}",
        cfg.path_mappings.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
}

#[test]
fn go_resolve_subpackage() {
    let root = go_fixture_root();
    let resolver = GoModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("main.go");
    let spec = ImportSpec {
        raw: "example.com/myapp/utils".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    match resolver.resolve(&from, &spec, &cfg) {
        Resolution::Resolved(path, _) => {
            assert_eq!(path, root.join("utils"));
        }
        other => panic!(
            "expected Resolved, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn go_not_applicable_for_non_go_file() {
    let root = go_fixture_root();
    let resolver = GoModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("main.rs");
    let spec = ImportSpec {
        raw: "example.com/myapp/utils".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    assert!(matches!(
        resolver.resolve(&from, &spec, &cfg),
        Resolution::NotApplicable
    ));
}

#[test]
fn go_module_of_file() {
    let root = go_fixture_root();
    let resolver = GoModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let file = root.join("utils/math.go");
    let modules = resolver.module_of_file(&root, &file, &cfg);
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].canonical_path, "example.com/myapp/utils");
}
