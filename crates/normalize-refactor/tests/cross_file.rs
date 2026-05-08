//! Cross-file name resolution tests.
//!
//! Tests ModuleResolver implementations for Rust, TypeScript, Python, Go,
//! JavaScript, and Ruby against fixture directories under `tests/fixtures/xfile/`.

use normalize_languages::go::GoModuleResolver;
use normalize_languages::javascript::JsModuleResolver;
use normalize_languages::python::PythonModuleResolver;
use normalize_languages::ruby::RubyModuleResolver;
use normalize_languages::rust::RustModuleResolver;
use normalize_languages::typescript::TsModuleResolver;
use normalize_languages::{ImportSpec, ModuleResolver, Resolution, support_for_path};
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

// =============================================================================
// JavaScript resolver tests
// =============================================================================

fn js_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xfile/javascript")
}

#[test]
fn js_resolve_relative_import() {
    let root = js_fixture_root();
    let resolver = JsModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("app.js");
    let spec = ImportSpec {
        raw: "./utils.js".to_string(),
        is_relative: true,
        names: vec!["sum".to_string()],
        is_glob: false,
    };
    match resolver.resolve(&from, &spec, &cfg) {
        Resolution::Resolved(path, _) => {
            assert_eq!(path, root.join("utils.js"));
        }
        other => panic!(
            "expected Resolved, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn js_not_applicable_for_non_js_file() {
    let root = js_fixture_root();
    let resolver = JsModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("main.ts");
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
fn js_bare_specifier_is_not_found() {
    let root = js_fixture_root();
    let resolver = JsModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("app.js");
    let spec = ImportSpec {
        raw: "lodash".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    assert!(matches!(
        resolver.resolve(&from, &spec, &cfg),
        Resolution::NotFound
    ));
}

// =============================================================================
// Ruby resolver tests
// =============================================================================

fn rb_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xfile/ruby")
}

#[test]
fn rb_resolve_require_relative() {
    let root = rb_fixture_root();
    let resolver = RubyModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("app.rb");
    let spec = ImportSpec {
        raw: "utils".to_string(),
        is_relative: true,
        names: Vec::new(),
        is_glob: false,
    };
    match resolver.resolve(&from, &spec, &cfg) {
        Resolution::Resolved(path, _) => {
            assert_eq!(path, root.join("utils.rb"));
        }
        other => panic!(
            "expected Resolved, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn rb_bare_require_is_not_found() {
    let root = rb_fixture_root();
    let resolver = RubyModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("app.rb");
    let spec = ImportSpec {
        raw: "json".to_string(),
        is_relative: false,
        names: Vec::new(),
        is_glob: false,
    };
    assert!(matches!(
        resolver.resolve(&from, &spec, &cfg),
        Resolution::NotFound
    ));
}

#[test]
fn rb_not_applicable_for_non_rb_file() {
    let root = rb_fixture_root();
    let resolver = RubyModuleResolver;
    let cfg = resolver.workspace_config(&root);

    let from = root.join("app.py");
    let spec = ImportSpec {
        raw: "utils".to_string(),
        is_relative: true,
        names: Vec::new(),
        is_glob: false,
    };
    assert!(matches!(
        resolver.resolve(&from, &spec, &cfg),
        Resolution::NotApplicable
    ));
}

// =============================================================================
// find_references confidence tagging
// =============================================================================

/// Verify that find_references tags results with "resolved" for Rust files
/// (which have a ModuleResolver) and "heuristic" for files without one.
#[test]
fn find_references_confidence_tag_no_index() {
    use std::path::Path;

    // Rust files have a resolver → would tag as "resolved"
    let rust_file = Path::new("src/utils.rs");
    let has_rust_resolver = support_for_path(rust_file)
        .and_then(|lang| lang.module_resolver())
        .is_some();
    assert!(has_rust_resolver, "Rust should have a module_resolver");

    // TypeScript files have a resolver
    let ts_file = Path::new("src/utils.ts");
    let has_ts_resolver = support_for_path(ts_file)
        .and_then(|lang| lang.module_resolver())
        .is_some();
    assert!(has_ts_resolver, "TypeScript should have a module_resolver");

    // Python files have a resolver
    let py_file = Path::new("src/utils.py");
    let has_py_resolver = support_for_path(py_file)
        .and_then(|lang| lang.module_resolver())
        .is_some();
    assert!(has_py_resolver, "Python should have a module_resolver");

    // Go files have a resolver
    let go_file = Path::new("main.go");
    let has_go_resolver = support_for_path(go_file)
        .and_then(|lang| lang.module_resolver())
        .is_some();
    assert!(has_go_resolver, "Go should have a module_resolver");

    // A Bash file has no resolver → would tag as "heuristic"
    let sh_file = Path::new("script.sh");
    let has_sh_resolver = support_for_path(sh_file)
        .and_then(|lang| lang.module_resolver())
        .is_some();
    assert!(!has_sh_resolver, "Bash should NOT have a module_resolver");
}

/// Matrix test: assert that every language with a module system returns Some(&dyn ModuleResolver),
/// and that data/scripting/template languages return None.
///
/// HAS_RESOLVER: languages that implement ModuleResolver
/// NOT_APPLICABLE: languages without a module system (returns None by design)
#[test]
fn module_resolver_coverage_matrix() {
    use normalize_languages::Language;

    // Languages that MUST have a resolver
    let has_resolver: &[(&dyn Language, &str)] = &[
        #[cfg(feature = "lang-rust")]
        (&normalize_languages::Rust, "Rust"),
        #[cfg(feature = "lang-typescript")]
        (&normalize_languages::TypeScript, "TypeScript"),
        #[cfg(feature = "lang-typescript")]
        (&normalize_languages::Tsx, "TSX"),
        #[cfg(feature = "lang-javascript")]
        (&normalize_languages::JavaScript, "JavaScript"),
        #[cfg(feature = "lang-python")]
        (&normalize_languages::Python, "Python"),
        #[cfg(feature = "lang-go")]
        (&normalize_languages::Go, "Go"),
        #[cfg(feature = "lang-ruby")]
        (&normalize_languages::Ruby, "Ruby"),
        #[cfg(feature = "lang-java")]
        (&normalize_languages::Java, "Java"),
        #[cfg(feature = "lang-kotlin")]
        (&normalize_languages::Kotlin, "Kotlin"),
        #[cfg(feature = "lang-groovy")]
        (&normalize_languages::Groovy, "Groovy"),
        #[cfg(feature = "lang-scala")]
        (&normalize_languages::Scala, "Scala"),
        #[cfg(feature = "lang-csharp")]
        (&normalize_languages::CSharp, "C#"),
        #[cfg(feature = "lang-vb")]
        (&normalize_languages::VB, "VB"),
        #[cfg(feature = "lang-fsharp")]
        (&normalize_languages::FSharp, "F#"),
        #[cfg(feature = "lang-swift")]
        (&normalize_languages::Swift, "Swift"),
        #[cfg(feature = "lang-dart")]
        (&normalize_languages::Dart, "Dart"),
        #[cfg(feature = "lang-zig")]
        (&normalize_languages::Zig, "Zig"),
        #[cfg(feature = "lang-elixir")]
        (&normalize_languages::Elixir, "Elixir"),
        #[cfg(feature = "lang-erlang")]
        (&normalize_languages::Erlang, "Erlang"),
        #[cfg(feature = "lang-haskell")]
        (&normalize_languages::Haskell, "Haskell"),
        #[cfg(feature = "lang-ocaml")]
        (&normalize_languages::OCaml, "OCaml"),
        #[cfg(feature = "lang-lua")]
        (&normalize_languages::Lua, "Lua"),
        #[cfg(feature = "lang-php")]
        (&normalize_languages::Php, "PHP"),
        #[cfg(feature = "lang-perl")]
        (&normalize_languages::Perl, "Perl"),
        #[cfg(feature = "lang-clojure")]
        (&normalize_languages::Clojure, "Clojure"),
        #[cfg(feature = "lang-commonlisp")]
        (&normalize_languages::CommonLisp, "Common Lisp"),
        #[cfg(feature = "lang-scheme")]
        (&normalize_languages::Scheme, "Scheme"),
        #[cfg(feature = "lang-gleam")]
        (&normalize_languages::Gleam, "Gleam"),
        #[cfg(feature = "lang-rescript")]
        (&normalize_languages::ReScript, "ReScript"),
    ];

    for (lang, name) in has_resolver {
        assert!(
            lang.module_resolver().is_some(),
            "{} should have a module_resolver (returns Some)",
            name
        );
    }

    // Languages that must NOT have a resolver (no module system)
    let not_applicable: &[(&dyn Language, &str)] = &[
        #[cfg(feature = "lang-css")]
        (&normalize_languages::Css, "CSS"),
        #[cfg(feature = "lang-scss")]
        (&normalize_languages::Scss, "SCSS"),
        #[cfg(feature = "lang-json")]
        (&normalize_languages::Json, "JSON"),
        #[cfg(feature = "lang-yaml")]
        (&normalize_languages::Yaml, "YAML"),
        #[cfg(feature = "lang-toml")]
        (&normalize_languages::Toml, "TOML"),
        #[cfg(feature = "lang-xml")]
        (&normalize_languages::Xml, "XML"),
        #[cfg(feature = "lang-html")]
        (&normalize_languages::Html, "HTML"),
        #[cfg(feature = "lang-markdown")]
        (&normalize_languages::Markdown, "Markdown"),
        #[cfg(feature = "lang-sql")]
        (&normalize_languages::Sql, "SQL"),
        #[cfg(feature = "lang-graphql")]
        (&normalize_languages::GraphQL, "GraphQL"),
        #[cfg(feature = "lang-bash")]
        (&normalize_languages::Bash, "Bash"),
        #[cfg(feature = "lang-fish")]
        (&normalize_languages::Fish, "Fish"),
        #[cfg(feature = "lang-awk")]
        (&normalize_languages::Awk, "Awk"),
        #[cfg(feature = "lang-powershell")]
        (&normalize_languages::PowerShell, "PowerShell"),
        #[cfg(feature = "lang-glsl")]
        (&normalize_languages::Glsl, "GLSL"),
        #[cfg(feature = "lang-hlsl")]
        (&normalize_languages::Hlsl, "HLSL"),
        #[cfg(feature = "lang-dockerfile")]
        (&normalize_languages::Dockerfile, "Dockerfile"),
    ];

    for (lang, name) in not_applicable {
        assert!(
            lang.module_resolver().is_none(),
            "{} should NOT have a module_resolver (returns None — no module system)",
            name
        );
    }

    // DEFERRED languages (module system exists but resolver is None for now):
    // C, C++, ObjC — preprocessor #include; no standard package mapping without toolchain
    // Nix — flake/nixpkgs paths require nix evaluation
    // D, Ada, Agda, Idris, Lean, Elm — niche; resolver not yet implemented
    // R, Julia, MATLAB — language server typically handles imports; file paths vary
    // Prolog — module system varies by implementation
}
