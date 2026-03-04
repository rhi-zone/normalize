//! Eval-backed manifest parsers (`feature = "eval"`).
//!
//! Each function runs a subprocess in the project root to extract dependency
//! information with full language-runtime fidelity (variables resolved,
//! conditionals evaluated). All functions return `None` on any failure so the
//! caller can fall back to the heuristic parser.

use std::path::Path;
use std::process::Command;

use crate::{DeclaredDep, DepKind, ParsedManifest};

/// Try to eval-parse the manifest at `root/filename`.
/// Returns `None` if no eval strategy is available for this filename, or if
/// the strategy fails (runtime absent, command error, parse error).
pub(crate) fn try_eval(filename: &str, root: &Path) -> Option<ParsedManifest> {
    match filename {
        "Package.swift" => eval_swift(root),
        "go.mod" => eval_go(root),
        "Gemfile" => eval_gemfile(root),
        "mix.exs" => eval_mix_exs(root),
        "setup.py" => eval_setup_py(root),
        "build.gradle" | "build.gradle.kts" => eval_gradle(root),
        "flake.nix" => eval_flake_nix(root),
        "conanfile.py" => eval_conanfile_py(root),
        _ => None,
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn run(program: &str, args: &[&str], root: &Path) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if out.status.success() {
        String::from_utf8(out.stdout).ok()
    } else {
        None
    }
}

// ── Swift — `swift package dump-package` ─────────────────────────────────────

fn eval_swift(root: &Path) -> Option<ParsedManifest> {
    let stdout = run("swift", &["package", "dump-package"], root)?;
    parse_swift_dump_json(&stdout)
}

fn parse_swift_dump_json(json: &str) -> Option<ParsedManifest> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;

    let name = v["name"].as_str().map(|s| s.to_string());

    let mut deps = Vec::new();

    for dep in v["dependencies"].as_array().unwrap_or(&vec![]) {
        // Each dependency is one of: sourceControl, fileSystem, registry
        if let Some(sc_list) = dep["sourceControl"].as_array() {
            for sc in sc_list {
                if let Some(d) = parse_swift_source_control(sc) {
                    deps.push(d);
                }
            }
        }
        // fileSystem deps (path deps) — skip, no useful version info
        // registry deps — uncommon, skip for now
    }

    Some(ParsedManifest {
        ecosystem: "spm",
        name,
        version: None,
        dependencies: deps,
    })
}

fn parse_swift_source_control(sc: &serde_json::Value) -> Option<DeclaredDep> {
    let identity = sc["identity"].as_str()?;
    let name = identity.to_string();

    let req = &sc["requirement"];

    let version_req = if let Some(ranges) = req["range"].as_array() {
        // [{"lowerBound": "x", "upperBound": "y"}]
        ranges.first().and_then(|r| {
            let lo = r["lowerBound"].as_str()?;
            let hi = r["upperBound"].as_str()?;
            Some(format!(">= {lo}, < {hi}"))
        })
    } else if let Some(exact) = req["exact"].as_array() {
        exact
            .first()
            .and_then(|e| e.as_str())
            .map(|s| format!("== {s}"))
    } else {
        // branch / revision — no semver constraint
        None
    };

    Some(DeclaredDep {
        name,
        version_req,
        kind: DepKind::Normal,
    })
}

// ── Go — `go mod edit -json` ──────────────────────────────────────────────────

fn eval_go(root: &Path) -> Option<ParsedManifest> {
    let stdout = run("go", &["mod", "edit", "-json"], root)?;
    parse_go_mod_json(&stdout)
}

fn parse_go_mod_json(json: &str) -> Option<ParsedManifest> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;

    let name = v["Module"]["Path"].as_str().map(|s| s.to_string());

    let mut deps = Vec::new();
    for req in v["Require"].as_array().unwrap_or(&vec![]) {
        let path = req["Path"].as_str()?;
        let ver = req["Version"].as_str().map(|s| s.to_string());
        let indirect = req["Indirect"].as_bool().unwrap_or(false);
        deps.push(DeclaredDep {
            name: path.to_string(),
            version_req: ver,
            kind: if indirect {
                DepKind::Optional
            } else {
                DepKind::Normal
            },
        });
    }

    Some(ParsedManifest {
        ecosystem: "go",
        name,
        version: None,
        dependencies: deps,
    })
}

// ── Ruby — `bundle exec ruby -e '…'` ─────────────────────────────────────────

const GEMFILE_RUBY: &str = r#"
require 'bundler'
require 'json'
begin
  d = Bundler.definition
  deps = d.dependencies.map do |dep|
    groups = dep.groups.map(&:to_s)
    kind = groups.any? { |g| %w[development test].include?(g) } ? 'dev' : 'normal'
    { name: dep.name, version: dep.requirement.to_s, kind: kind }
  end
  STDOUT.puts JSON.generate(deps)
rescue => e
  STDERR.puts e.message
  exit 1
end
"#;

fn eval_gemfile(root: &Path) -> Option<ParsedManifest> {
    let stdout = run("bundle", &["exec", "ruby", "-e", GEMFILE_RUBY], root)?;
    parse_gemfile_json(&stdout)
}

fn parse_gemfile_json(json: &str) -> Option<ParsedManifest> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(json.trim()).ok()?;

    let deps = arr
        .into_iter()
        .filter_map(|v| {
            let name = v["name"].as_str()?.to_string();
            let version_req = v["version"].as_str().and_then(|s| {
                // ">= 0" is the default no-constraint marker in Bundler
                if s == ">= 0" {
                    None
                } else {
                    Some(s.to_string())
                }
            });
            let kind = if v["kind"].as_str() == Some("dev") {
                DepKind::Dev
            } else {
                DepKind::Normal
            };
            Some(DeclaredDep {
                name,
                version_req,
                kind,
            })
        })
        .collect();

    Some(ParsedManifest {
        ecosystem: "bundler",
        name: None,
        version: None,
        dependencies: deps,
    })
}

// ── Elixir — `elixir -e '…'` ──────────────────────────────────────────────────

// Elixir script that loads mix.exs without a full Mix project context and
// extracts the deps list. Emits JSON to stdout.
// We use :erlang.term_to_binary / inspect rather than Jason to avoid requiring
// Jason to be installed in the project.
const MIX_ELIXIR: &str = r##"
Code.eval_file("mix.exs")
config =
  try do
    Mix.Project.config()
  rescue
    _ -> []
  end
deps = Keyword.get(config, :deps, [])
result =
  Enum.flat_map(deps, fn
    {name, version} when is_binary(version) ->
      [%{name: name, version: version, kind: "normal"}]
    {name, opts} when is_list(opts) ->
      only = opts[:only]
      kind =
        cond do
          only in [:dev, :test, :docs] -> "dev"
          is_list(only) -> "dev"
          true -> "normal"
        end
      ver = opts[:version]
      [%{name: name, version: (if is_binary(ver), do: ver, else: nil), kind: kind}]
    _ ->
      []
  end)
json =
  Enum.map_join(result, ",", fn %{name: n, version: v, kind: k} ->
    ver_part = if v, do: ~s("version":"#{v}"), else: ~s("version":null)
    ~s({"name":"#{n}",#{ver_part},"kind":"#{k}"})
  end)
IO.puts("[#{json}]")
"##;

fn eval_mix_exs(root: &Path) -> Option<ParsedManifest> {
    let stdout = run("elixir", &["-e", MIX_ELIXIR], root)?;
    parse_mix_json(&stdout)
}

fn parse_mix_json(json: &str) -> Option<ParsedManifest> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(json.trim()).ok()?;

    let deps = arr
        .into_iter()
        .filter_map(|v| {
            let name = v["name"].as_str()?.to_string();
            let version_req = v["version"].as_str().map(|s| s.to_string());
            let kind = if v["kind"].as_str() == Some("dev") {
                DepKind::Dev
            } else {
                DepKind::Normal
            };
            Some(DeclaredDep {
                name,
                version_req,
                kind,
            })
        })
        .collect();

    Some(ParsedManifest {
        ecosystem: "hex",
        name: None,
        version: None,
        dependencies: deps,
    })
}

// ── Python — `python3 -c '…'` / `python -c '…'` ──────────────────────────────

const SETUP_PY_SCRIPT: &str = r#"import sys, json
sys.argv = ['setup.py']  # prevent argparse surprises

captured = {}

def _mock_setup(**kw):
    captured.update(kw)

# Patch both setuptools and distutils before importing setup.py
import types
_fake = types.ModuleType('setuptools')
_fake.setup = _mock_setup
_fake.find_packages = lambda *a, **kw: []
_fake.find_namespace_packages = lambda *a, **kw: []
sys.modules['setuptools'] = _fake

try:
    import distutils.core as _dc
    _dc.setup = _mock_setup
except Exception:
    pass

try:
    with open('setup.py') as _f:
        exec(compile(_f.read(), 'setup.py', 'exec'), {'__name__': '__main__'})
except SystemExit:
    pass
except Exception as e:
    sys.stderr.write(str(e) + '\n')

def _parse_req(r):
    import re
    m = re.match(r'([A-Za-z0-9_.\-]+)(.*)', r.strip())
    if not m:
        return None
    return {'name': m.group(1).replace('-','_').lower(), 'version': m.group(2).strip() or None}

deps = []
for r in captured.get('install_requires', []):
    p = _parse_req(r)
    if p:
        deps.append({'name': p['name'], 'version': p['version'], 'kind': 'normal'})
for r in captured.get('tests_require', []):
    p = _parse_req(r)
    if p:
        deps.append({'name': p['name'], 'version': p['version'], 'kind': 'dev'})
for grp, reqs in (captured.get('extras_require', None) or {}).items():
    kind = 'dev' if grp in ('dev','test','testing','tests','develop','development') else 'optional'
    for r in (reqs or []):
        p = _parse_req(r)
        if p:
            deps.append({'name': p['name'], 'version': p['version'], 'kind': kind})

print(json.dumps({
    'name': captured.get('name'),
    'version': captured.get('version'),
    'deps': deps,
}))
"#;

fn eval_setup_py(root: &Path) -> Option<ParsedManifest> {
    // Try python3 first, fall back to python
    let stdout = run("python3", &["-c", SETUP_PY_SCRIPT], root)
        .or_else(|| run("python", &["-c", SETUP_PY_SCRIPT], root))?;
    parse_setup_py_json(&stdout)
}

fn parse_setup_py_json(json: &str) -> Option<ParsedManifest> {
    let v: serde_json::Value = serde_json::from_str(json.trim()).ok()?;

    let name = v["name"].as_str().map(|s| s.to_string());
    let version = v["version"].as_str().map(|s| s.to_string());

    let mut deps = Vec::new();
    for dep in v["deps"].as_array().unwrap_or(&vec![]) {
        let dep_name = dep["name"].as_str()?.to_string();
        let version_req = dep["version"].as_str().map(|s| s.to_string());
        let kind = match dep["kind"].as_str() {
            Some("dev") => DepKind::Dev,
            Some("optional") => DepKind::Optional,
            _ => DepKind::Normal,
        };
        deps.push(DeclaredDep {
            name: dep_name,
            version_req,
            kind,
        });
    }

    Some(ParsedManifest {
        ecosystem: "python",
        name,
        version,
        dependencies: deps,
    })
}

// ── Gradle — init script injection ───────────────────────────────────────────

const GRADLE_INIT_SCRIPT: &str = r#"
allprojects {
    task __normalizeDepsJson {
        doLast {
            def result = []
            configurations.each { config ->
                try {
                    config.resolvedConfiguration.resolvedArtifacts.each { a ->
                        def n = a.moduleVersion.id
                        def isTest = config.name.toLowerCase().contains('test')
                        result << [name: "${n.group}:${n.name}", version: n.version, kind: isTest ? 'dev' : 'normal']
                    }
                } catch (ignored) {}
            }
            // deduplicate by name
            def seen = [] as Set
            def deduped = result.findAll { seen.add(it.name) }
            println groovy.json.JsonOutput.toJson(deduped)
        }
    }
}
"#;

fn eval_gradle(root: &Path) -> Option<ParsedManifest> {
    // Write the init script to a temp file
    let mut init_path = std::env::temp_dir();
    init_path.push("normalize_gradle_init.groovy");
    std::fs::write(&init_path, GRADLE_INIT_SCRIPT).ok()?;

    let init_arg = init_path.to_str()?;

    // Try ./gradlew first (wrapper), fall back to gradle
    let stdout = run(
        "./gradlew",
        &[
            "-I",
            init_arg,
            "--quiet",
            "--no-daemon",
            ":__normalizeDepsJson",
        ],
        root,
    )
    .or_else(|| {
        run(
            "gradle",
            &[
                "-I",
                init_arg,
                "--quiet",
                "--no-daemon",
                ":__normalizeDepsJson",
            ],
            root,
        )
    })?;

    parse_gradle_json(&stdout)
}

fn parse_gradle_json(json: &str) -> Option<ParsedManifest> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(json.trim()).ok()?;

    let deps = arr
        .into_iter()
        .filter_map(|v| {
            let name = v["name"].as_str()?.to_string();
            let version_req = v["version"].as_str().map(|s| s.to_string());
            let kind = if v["kind"].as_str() == Some("dev") {
                DepKind::Dev
            } else {
                DepKind::Normal
            };
            Some(DeclaredDep {
                name,
                version_req,
                kind,
            })
        })
        .collect();

    Some(ParsedManifest {
        ecosystem: "gradle",
        name: None,
        version: None,
        dependencies: deps,
    })
}

// ── Nix — `nix flake metadata --json` ────────────────────────────────────────

fn eval_flake_nix(root: &Path) -> Option<ParsedManifest> {
    let stdout = run("nix", &["flake", "metadata", "--json"], root)?;
    parse_flake_metadata_json(&stdout)
}

fn parse_flake_metadata_json(json: &str) -> Option<ParsedManifest> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;

    let nodes = v["locks"]["nodes"].as_object()?;

    let mut deps = Vec::new();
    for (key, node) in nodes {
        // Skip the root node (has "inputs" but no "locked")
        let locked = &node["locked"];
        if locked.is_null() || !locked.is_object() {
            continue;
        }
        // Skip nodes without a "type" field in locked (shouldn't happen but be safe)
        if locked["type"].as_str().is_none() {
            continue;
        }

        // Version: prefer "version" field, fall back to short rev (first 7 chars)
        let version_req = locked["version"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| {
                locked["rev"]
                    .as_str()
                    .map(|rev| rev.chars().take(7).collect())
            });

        deps.push(DeclaredDep {
            name: key.clone(),
            version_req,
            kind: DepKind::Normal,
        });
    }

    Some(ParsedManifest {
        ecosystem: "nix",
        name: None,
        version: None,
        dependencies: deps,
    })
}

// ── Conan — mock ConanFile approach ──────────────────────────────────────────

const CONAN_PY_SCRIPT: &str = r#"import sys, json, types

captured_requires = []
captured_build_requires = []
captured_name = None
captured_version = None

class FakeConanFile:
    settings = None
    options = {}
    default_options = {}
    generators = []
    exports = []
    exports_sources = []

    def __init__(self):
        pass

    def requirements(self):
        pass

    def build_requirements(self):
        pass

class _Meta(type):
    def __new__(mcs, name, bases, namespace):
        cls = super().__new__(mcs, name, bases, namespace)
        if name != 'ConanFile' and any(b.__name__ == 'ConanFile' for b in bases):
            global captured_name, captured_version, captured_requires, captured_build_requires
            captured_name = getattr(cls, 'name', None)
            captured_version = getattr(cls, 'version', None)
            requires = getattr(cls, 'requires', None)
            build_requires = getattr(cls, 'build_requires', None)
            if isinstance(requires, str):
                captured_requires = [requires]
            elif isinstance(requires, (list, tuple)):
                captured_requires = list(requires)
            if isinstance(build_requires, str):
                captured_build_requires = [build_requires]
            elif isinstance(build_requires, (list, tuple)):
                captured_build_requires = list(build_requires)
        return cls

class ConanFile(metaclass=_Meta):
    pass

conan_mod = types.ModuleType('conans')
conan_mod.ConanFile = ConanFile
conan_mod.tools = types.ModuleType('tools')
sys.modules['conans'] = conan_mod
sys.modules['conan'] = conan_mod

# Also handle 'from conan import ConanFile' style
conan2 = types.ModuleType('conan')
conan2.ConanFile = ConanFile
sys.modules['conan'] = conan2

try:
    with open('conanfile.py') as f:
        exec(compile(f.read(), 'conanfile.py', 'exec'), {'__name__': '__main__'})
except Exception as e:
    sys.stderr.write(str(e) + '\n')

def parse_ref(r):
    # "pkg/1.0@user/channel" or "pkg/1.0" or "pkg"
    r = r.strip()
    if '/' in r:
        parts = r.split('/', 1)
        name = parts[0]
        ver_part = parts[1].split('@')[0]
        return {'name': name, 'version': ver_part or None}
    return {'name': r, 'version': None}

deps = []
for r in captured_requires:
    p = parse_ref(r)
    deps.append({'name': p['name'], 'version': p['version'], 'kind': 'normal'})
for r in captured_build_requires:
    p = parse_ref(r)
    deps.append({'name': p['name'], 'version': p['version'], 'kind': 'build'})

print(json.dumps({
    'name': captured_name,
    'version': captured_version,
    'deps': deps,
}))
"#;

fn eval_conanfile_py(root: &Path) -> Option<ParsedManifest> {
    // Try python3 first, fall back to python
    let stdout = run("python3", &["-c", CONAN_PY_SCRIPT], root)
        .or_else(|| run("python", &["-c", CONAN_PY_SCRIPT], root))?;
    parse_conan_py_json(&stdout)
}

fn parse_conan_py_json(json: &str) -> Option<ParsedManifest> {
    let v: serde_json::Value = serde_json::from_str(json.trim()).ok()?;

    let name = v["name"].as_str().map(|s| s.to_string());
    let version = v["version"].as_str().map(|s| s.to_string());

    let mut deps = Vec::new();
    for dep in v["deps"].as_array().unwrap_or(&vec![]) {
        let dep_name = dep["name"].as_str()?.to_string();
        let version_req = dep["version"].as_str().map(|s| s.to_string());
        let kind = match dep["kind"].as_str() {
            Some("build") => DepKind::Build,
            Some("dev") => DepKind::Dev,
            _ => DepKind::Normal,
        };
        deps.push(DeclaredDep {
            name: dep_name,
            version_req,
            kind,
        });
    }

    Some(ParsedManifest {
        ecosystem: "conan",
        name,
        version,
        dependencies: deps,
    })
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_swift_dump_json() {
        // Abbreviated output from `swift package dump-package` on vapor
        let json = r#"{
            "name": "vapor",
            "dependencies": [
                {
                    "sourceControl": [{
                        "identity": "swift-nio",
                        "location": {"remote": [{"urlString": "https://github.com/apple/swift-nio.git"}]},
                        "requirement": {
                            "range": [{"lowerBound": "2.81.0", "upperBound": "3.0.0"}]
                        }
                    }]
                },
                {
                    "sourceControl": [{
                        "identity": "swift-crypto",
                        "location": {"remote": [{"urlString": "https://github.com/apple/swift-crypto.git"}]},
                        "requirement": {
                            "range": [{"lowerBound": "1.0.0", "upperBound": "5.0.0"}]
                        }
                    }]
                },
                {
                    "sourceControl": [{
                        "identity": "async-kit",
                        "location": {"remote": [{"urlString": "https://github.com/vapor/async-kit.git"}]},
                        "requirement": {
                            "exact": ["1.15.0"]
                        }
                    }]
                }
            ]
        }"#;

        let m = parse_swift_dump_json(json).unwrap();
        assert_eq!(m.ecosystem, "spm");
        assert_eq!(m.name.as_deref(), Some("vapor"));
        assert_eq!(m.dependencies.len(), 3);

        let nio = m
            .dependencies
            .iter()
            .find(|d| d.name == "swift-nio")
            .unwrap();
        assert_eq!(nio.version_req.as_deref(), Some(">= 2.81.0, < 3.0.0"));

        let crypto = m
            .dependencies
            .iter()
            .find(|d| d.name == "swift-crypto")
            .unwrap();
        assert_eq!(crypto.version_req.as_deref(), Some(">= 1.0.0, < 5.0.0"));

        let kit = m
            .dependencies
            .iter()
            .find(|d| d.name == "async-kit")
            .unwrap();
        assert_eq!(kit.version_req.as_deref(), Some("== 1.15.0"));
    }

    #[test]
    fn test_parse_go_mod_json() {
        let json = r#"{
            "Module": {"Path": "github.com/spf13/cobra"},
            "Go": "1.15",
            "Require": [
                {"Path": "github.com/spf13/pflag", "Version": "v1.0.9"},
                {"Path": "github.com/inconshreveable/mousetrap", "Version": "v1.1.0"},
                {"Path": "golang.org/x/sys", "Version": "v0.0.1", "Indirect": true}
            ]
        }"#;

        let m = parse_go_mod_json(json).unwrap();
        assert_eq!(m.ecosystem, "go");
        assert_eq!(m.name.as_deref(), Some("github.com/spf13/cobra"));

        let pflag = m
            .dependencies
            .iter()
            .find(|d| d.name == "github.com/spf13/pflag")
            .unwrap();
        assert_eq!(pflag.version_req.as_deref(), Some("v1.0.9"));
        assert_eq!(pflag.kind, DepKind::Normal);

        let sys = m
            .dependencies
            .iter()
            .find(|d| d.name == "golang.org/x/sys")
            .unwrap();
        assert_eq!(sys.kind, DepKind::Optional); // indirect
    }

    #[test]
    fn test_parse_gemfile_json() {
        let json = r#"[
            {"name": "rake", "version": ">= 0", "kind": "normal"},
            {"name": "minitest", "version": "~> 5.0", "kind": "normal"},
            {"name": "rspec", "version": ">= 0", "kind": "dev"},
            {"name": "nokogiri", "version": "> 1.5.0", "kind": "normal"}
        ]"#;

        let m = parse_gemfile_json(json).unwrap();
        assert_eq!(m.ecosystem, "bundler");

        let rake = m.dependencies.iter().find(|d| d.name == "rake").unwrap();
        assert!(rake.version_req.is_none()); // ">= 0" stripped

        let minitest = m
            .dependencies
            .iter()
            .find(|d| d.name == "minitest")
            .unwrap();
        assert_eq!(minitest.version_req.as_deref(), Some("~> 5.0"));

        let rspec = m.dependencies.iter().find(|d| d.name == "rspec").unwrap();
        assert_eq!(rspec.kind, DepKind::Dev);

        let noko = m
            .dependencies
            .iter()
            .find(|d| d.name == "nokogiri")
            .unwrap();
        assert_eq!(noko.version_req.as_deref(), Some("> 1.5.0"));
    }

    #[test]
    fn test_parse_setup_py_json() {
        let json = r#"{
            "name": "mypackage",
            "version": "1.0.0",
            "deps": [
                {"name": "requests", "version": ">=2.28.0", "kind": "normal"},
                {"name": "click", "version": ">=8.0", "kind": "normal"},
                {"name": "pytest", "version": ">=7.0", "kind": "dev"},
                {"name": "black", "version": null, "kind": "dev"},
                {"name": "sphinx", "version": ">=5.0", "kind": "optional"}
            ]
        }"#;

        let m = parse_setup_py_json(json).unwrap();
        assert_eq!(m.ecosystem, "python");
        assert_eq!(m.name.as_deref(), Some("mypackage"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));
        assert_eq!(m.dependencies.len(), 5);

        let req = m
            .dependencies
            .iter()
            .find(|d| d.name == "requests")
            .unwrap();
        assert_eq!(req.version_req.as_deref(), Some(">=2.28.0"));
        assert_eq!(req.kind, DepKind::Normal);

        let pytest = m.dependencies.iter().find(|d| d.name == "pytest").unwrap();
        assert_eq!(pytest.kind, DepKind::Dev);
        assert_eq!(pytest.version_req.as_deref(), Some(">=7.0"));

        let black = m.dependencies.iter().find(|d| d.name == "black").unwrap();
        assert_eq!(black.kind, DepKind::Dev);
        assert!(black.version_req.is_none());

        let sphinx = m.dependencies.iter().find(|d| d.name == "sphinx").unwrap();
        assert_eq!(sphinx.kind, DepKind::Optional);
    }

    #[test]
    fn test_parse_setup_py_json_minimal() {
        // name/version null, empty deps
        let json = r#"{"name": null, "version": null, "deps": []}"#;
        let m = parse_setup_py_json(json).unwrap();
        assert_eq!(m.ecosystem, "python");
        assert!(m.name.is_none());
        assert!(m.version.is_none());
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_parse_gradle_json() {
        let json = r#"[
            {"name": "com.google.guava:guava", "version": "32.1.2-jre", "kind": "normal"},
            {"name": "org.springframework:spring-core", "version": "6.0.11", "kind": "normal"},
            {"name": "junit:junit", "version": "4.13.2", "kind": "dev"}
        ]"#;

        let m = parse_gradle_json(json).unwrap();
        assert_eq!(m.ecosystem, "gradle");
        assert!(m.name.is_none());
        assert!(m.version.is_none());
        assert_eq!(m.dependencies.len(), 3);

        let guava = m
            .dependencies
            .iter()
            .find(|d| d.name == "com.google.guava:guava")
            .unwrap();
        assert_eq!(guava.version_req.as_deref(), Some("32.1.2-jre"));
        assert_eq!(guava.kind, DepKind::Normal);

        let junit = m
            .dependencies
            .iter()
            .find(|d| d.name == "junit:junit")
            .unwrap();
        assert_eq!(junit.kind, DepKind::Dev);
    }

    #[test]
    fn test_parse_flake_metadata_json() {
        let json = r#"{
            "locks": {
                "nodes": {
                    "root": {
                        "inputs": {
                            "nixpkgs": "nixpkgs",
                            "flake-utils": "flake-utils"
                        }
                    },
                    "nixpkgs": {
                        "locked": {
                            "lastModified": 1700000000,
                            "narHash": "sha256-abc",
                            "owner": "NixOS",
                            "repo": "nixpkgs",
                            "rev": "abcdef1234567890",
                            "type": "github"
                        }
                    },
                    "flake-utils": {
                        "locked": {
                            "lastModified": 1699000000,
                            "narHash": "sha256-xyz",
                            "owner": "numtide",
                            "repo": "flake-utils",
                            "rev": "1122334455667788",
                            "type": "github"
                        }
                    }
                }
            }
        }"#;

        let m = parse_flake_metadata_json(json).unwrap();
        assert_eq!(m.ecosystem, "nix");
        assert!(m.name.is_none());
        // root node must be skipped
        assert_eq!(m.dependencies.len(), 2);

        let nixpkgs = m.dependencies.iter().find(|d| d.name == "nixpkgs").unwrap();
        assert_eq!(nixpkgs.version_req.as_deref(), Some("abcdef1")); // short rev
        assert_eq!(nixpkgs.kind, DepKind::Normal);

        let utils = m
            .dependencies
            .iter()
            .find(|d| d.name == "flake-utils")
            .unwrap();
        assert_eq!(utils.version_req.as_deref(), Some("1122334"));
    }

    #[test]
    fn test_parse_flake_metadata_json_with_version() {
        // Node with a "version" field (e.g., fetched tarball with explicit version)
        let json = r#"{
            "locks": {
                "nodes": {
                    "root": {
                        "inputs": {"crane": "crane"}
                    },
                    "crane": {
                        "locked": {
                            "type": "github",
                            "owner": "ipetkov",
                            "repo": "crane",
                            "rev": "deadbeef00000000",
                            "version": "0.16.3"
                        }
                    }
                }
            }
        }"#;

        let m = parse_flake_metadata_json(json).unwrap();
        let crane = m.dependencies.iter().find(|d| d.name == "crane").unwrap();
        // "version" field preferred over rev
        assert_eq!(crane.version_req.as_deref(), Some("0.16.3"));
    }

    #[test]
    fn test_parse_conan_py_json() {
        let json = r#"{
            "name": "mylib",
            "version": "1.2.3",
            "deps": [
                {"name": "boost", "version": "1.82.0", "kind": "normal"},
                {"name": "zlib", "version": "1.3", "kind": "normal"},
                {"name": "cmake", "version": "3.25.0", "kind": "build"},
                {"name": "gtest", "version": null, "kind": "normal"}
            ]
        }"#;

        let m = parse_conan_py_json(json).unwrap();
        assert_eq!(m.ecosystem, "conan");
        assert_eq!(m.name.as_deref(), Some("mylib"));
        assert_eq!(m.version.as_deref(), Some("1.2.3"));
        assert_eq!(m.dependencies.len(), 4);

        let boost = m.dependencies.iter().find(|d| d.name == "boost").unwrap();
        assert_eq!(boost.version_req.as_deref(), Some("1.82.0"));
        assert_eq!(boost.kind, DepKind::Normal);

        let cmake = m.dependencies.iter().find(|d| d.name == "cmake").unwrap();
        assert_eq!(cmake.version_req.as_deref(), Some("3.25.0"));
        assert_eq!(cmake.kind, DepKind::Build);

        let gtest = m.dependencies.iter().find(|d| d.name == "gtest").unwrap();
        assert!(gtest.version_req.is_none());
        assert_eq!(gtest.kind, DepKind::Normal);
    }

    #[test]
    fn test_parse_mix_json() {
        let json = r#"[
            {"name": "phoenix", "version": "~> 1.7", "kind": "normal"},
            {"name": "ex_doc", "version": "~> 0.38", "kind": "dev"},
            {"name": "postgrex", "version": null, "kind": "normal"}
        ]"#;

        let m = parse_mix_json(json).unwrap();
        assert_eq!(m.ecosystem, "hex");

        let phoenix = m.dependencies.iter().find(|d| d.name == "phoenix").unwrap();
        assert_eq!(phoenix.version_req.as_deref(), Some("~> 1.7"));
        assert_eq!(phoenix.kind, DepKind::Normal);

        let ex_doc = m.dependencies.iter().find(|d| d.name == "ex_doc").unwrap();
        assert_eq!(ex_doc.kind, DepKind::Dev);

        let pg = m
            .dependencies
            .iter()
            .find(|d| d.name == "postgrex")
            .unwrap();
        assert!(pg.version_req.is_none());
    }
}
