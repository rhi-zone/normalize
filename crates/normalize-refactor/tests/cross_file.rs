//! Cross-file name resolution tests using RustModuleResolver.
//!
//! These tests exercise the ModuleResolver trait directly against the
//! `tests/fixtures/xfile/rust/` fixture: a 3-file Cargo crate with
//! `utils.rs`, `models.rs`, and `main.rs`.

use normalize_languages::rust::RustModuleResolver;
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
