//! Real-world manifest fixture tests.
//!
//! Each test parses an actual manifest file fetched verbatim from a permissively-licensed
//! open-source project and asserts that known dependencies are extracted correctly.

use normalize_manifest::{DepKind, parse_manifest, parse_manifest_by_extension};

fn fixture(path: &str) -> String {
    let full = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("can't read {full}: {e}"))
}

// ── Go ──────────────────────────────────────────────────────────────────────

#[test]
fn cobra_go_mod() {
    let content = fixture("cobra/go.mod");
    let m = parse_manifest("go.mod", &content).unwrap();
    assert_eq!(m.ecosystem, "go");
    assert_eq!(m.name.as_deref(), Some("github.com/spf13/cobra"));

    let pflag = m
        .dependencies
        .iter()
        .find(|d| d.name == "github.com/spf13/pflag")
        .unwrap();
    assert_eq!(pflag.version_req.as_deref(), Some("v1.0.9"));
    assert_eq!(pflag.kind, DepKind::Normal);

    assert!(
        m.dependencies
            .iter()
            .any(|d| d.name == "github.com/inconshreveable/mousetrap")
    );
    assert!(
        m.dependencies
            .iter()
            .any(|d| d.name == "github.com/cpuguy83/go-md2man/v2")
    );
}

// ── Node.js ──────────────────────────────────────────────────────────────────

#[test]
fn express_package_json() {
    let content = fixture("express/package.json");
    let m = parse_manifest("package.json", &content).unwrap();
    assert_eq!(m.ecosystem, "npm");
    assert_eq!(m.name.as_deref(), Some("express"));

    let body_parser = m
        .dependencies
        .iter()
        .find(|d| d.name == "body-parser")
        .unwrap();
    assert_eq!(body_parser.kind, DepKind::Normal);

    let mocha = m.dependencies.iter().find(|d| d.name == "mocha").unwrap();
    assert_eq!(mocha.kind, DepKind::Dev);

    // Should have both prod and dev deps
    assert!(
        m.dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .count()
            > 10
    );
    assert!(
        m.dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .count()
            > 5
    );
}

// ── PHP ──────────────────────────────────────────────────────────────────────

#[test]
fn slim_composer_json() {
    let content = fixture("slim/composer.json");
    let m = parse_manifest("composer.json", &content).unwrap();
    assert_eq!(m.ecosystem, "composer");

    // php/ext-* platform requirements are filtered out
    assert!(!m.dependencies.iter().any(|d| d.name == "php"));
    assert!(!m.dependencies.iter().any(|d| d.name.starts_with("ext-")));

    let fast_route = m
        .dependencies
        .iter()
        .find(|d| d.name == "nikic/fast-route")
        .unwrap();
    assert_eq!(fast_route.kind, DepKind::Normal);

    let phpunit = m
        .dependencies
        .iter()
        .find(|d| d.name == "phpunit/phpunit")
        .unwrap();
    assert_eq!(phpunit.kind, DepKind::Dev);
}

// ── Python ───────────────────────────────────────────────────────────────────

#[test]
fn flask_pyproject_toml() {
    let content = fixture("flask/pyproject.toml");
    let m = parse_manifest("pyproject.toml", &content).unwrap();
    assert_eq!(m.ecosystem, "python");
    assert_eq!(m.name.as_deref(), Some("Flask"));

    let click = m.dependencies.iter().find(|d| d.name == "click").unwrap();
    assert_eq!(click.kind, DepKind::Normal);
    assert!(click.version_req.is_some());

    let jinja2 = m.dependencies.iter().find(|d| d.name == "jinja2").unwrap();
    assert_eq!(jinja2.kind, DepKind::Normal);

    let werkzeug = m
        .dependencies
        .iter()
        .find(|d| d.name == "werkzeug")
        .unwrap();
    assert_eq!(werkzeug.kind, DepKind::Normal);
}

// ── Elixir ───────────────────────────────────────────────────────────────────

#[test]
fn phoenix_mix_exs() {
    let content = fixture("phoenix/mix.exs");
    let m = parse_manifest("mix.exs", &content).unwrap();
    assert_eq!(m.ecosystem, "hex");
    assert_eq!(m.name.as_deref(), Some("phoenix"));

    let plug = m.dependencies.iter().find(|d| d.name == "plug").unwrap();
    assert_eq!(plug.kind, DepKind::Normal);

    // ex_doc is only: :docs — Dev
    let ex_doc = m.dependencies.iter().find(|d| d.name == "ex_doc").unwrap();
    assert_eq!(ex_doc.kind, DepKind::Dev);
}

// ── Ruby ─────────────────────────────────────────────────────────────────────

#[test]
fn sinatra_gemfile() {
    let content = fixture("sinatra/Gemfile");
    let m = parse_manifest("Gemfile", &content).unwrap();
    assert_eq!(m.ecosystem, "bundler");

    // Simple bare gems
    assert!(m.dependencies.iter().any(|d| d.name == "rake"));
    assert!(m.dependencies.iter().any(|d| d.name == "rackup"));

    // minitest has a version constraint
    let minitest = m
        .dependencies
        .iter()
        .find(|d| d.name == "minitest")
        .unwrap();
    assert!(
        minitest
            .version_req
            .as_deref()
            .unwrap_or("")
            .contains("5.0")
    );

    // rack, puma etc. appear even though version is a Ruby variable — gem name extracted
    assert!(m.dependencies.iter().any(|d| d.name == "rack"));
    assert!(m.dependencies.iter().any(|d| d.name == "puma"));
}

// ── Dart / Flutter ───────────────────────────────────────────────────────────

#[test]
fn dart_http_pubspec_yaml() {
    let content = fixture("dart-http/pubspec.yaml");
    let m = parse_manifest("pubspec.yaml", &content).unwrap();
    assert_eq!(m.ecosystem, "pub");
    assert_eq!(m.name.as_deref(), Some("http"));

    let async_dep = m.dependencies.iter().find(|d| d.name == "async").unwrap();
    assert_eq!(async_dep.kind, DepKind::Normal);
    assert!(async_dep.version_req.is_some());

    let test_dep = m.dependencies.iter().find(|d| d.name == "test").unwrap();
    assert_eq!(test_dep.kind, DepKind::Dev);

    // http_client_conformance_tests is a path dep — should still appear or be absent but not panic
    // (path deps have no version_req)
}

// ── Swift ────────────────────────────────────────────────────────────────────

#[test]
fn vapor_package_swift() {
    let content = fixture("vapor/Package.swift");
    let m = parse_manifest("Package.swift", &content).unwrap();
    assert_eq!(m.ecosystem, "spm");
    assert_eq!(m.name.as_deref(), Some("vapor"));

    let swift_nio = m
        .dependencies
        .iter()
        .find(|d| d.name == "swift-nio")
        .unwrap();
    assert_eq!(swift_nio.kind, DepKind::Normal);
    assert!(
        swift_nio
            .version_req
            .as_deref()
            .unwrap_or("")
            .starts_with(">=")
    );

    let swift_crypto = m
        .dependencies
        .iter()
        .find(|d| d.name == "swift-crypto")
        .unwrap();
    assert!(swift_crypto.version_req.is_some());

    // Many deps — all Normal (no dev/test scope in Package.swift)
    assert!(m.dependencies.len() > 10);
}

// ── Lua ──────────────────────────────────────────────────────────────────────

#[test]
fn luasocket_rockspec() {
    let content = fixture("luasocket/luasocket-3.1.0-1.rockspec");
    let m = parse_manifest_by_extension("luasocket-3.1.0-1.rockspec", &content).unwrap();
    assert_eq!(m.ecosystem, "luarocks");
    assert_eq!(m.name.as_deref(), Some("LuaSocket"));
    assert_eq!(m.version.as_deref(), Some("3.1.0-1"));

    // Only dependency is "lua >= 5.1" which is filtered as the runtime
    assert!(
        m.dependencies.is_empty(),
        "lua runtime dep should be filtered"
    );
}

// ── Haskell ──────────────────────────────────────────────────────────────────

#[test]
fn servant_cabal() {
    let content = fixture("servant/servant.cabal");
    let m = parse_manifest_by_extension("servant.cabal", &content).unwrap();
    assert_eq!(m.ecosystem, "cabal");
    assert_eq!(m.name.as_deref(), Some("servant"));

    // base is filtered
    assert!(!m.dependencies.iter().any(|d| d.name == "base"));

    let aeson = m.dependencies.iter().find(|d| d.name == "aeson").unwrap();
    assert_eq!(aeson.kind, DepKind::Normal);

    // hspec is only in test-suite
    let hspec = m.dependencies.iter().find(|d| d.name == "hspec").unwrap();
    assert_eq!(hspec.kind, DepKind::Dev);
}

// ── Java / Maven ─────────────────────────────────────────────────────────────

#[test]
fn commons_lang3_pom_xml() {
    let content = fixture("commons-lang3/pom.xml");
    let m = parse_manifest("pom.xml", &content).unwrap();
    assert_eq!(m.ecosystem, "maven");

    // All testing deps are scoped "test" → Dev
    let junit = m
        .dependencies
        .iter()
        .find(|d| d.name.contains("junit-jupiter"))
        .unwrap();
    assert_eq!(junit.kind, DepKind::Dev);

    let easymock = m
        .dependencies
        .iter()
        .find(|d| d.name.contains("easymock"))
        .unwrap();
    assert_eq!(easymock.kind, DepKind::Dev);

    // mockito-inline has no scope → Normal
    let mockito = m
        .dependencies
        .iter()
        .find(|d| d.name.contains("mockito"))
        .unwrap();
    assert_eq!(mockito.kind, DepKind::Normal);
}
