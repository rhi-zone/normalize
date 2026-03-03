//! Parsers for Gradle build files.
//!
//! - `build.gradle` (Groovy DSL)
//! - `build.gradle.kts` (Kotlin DSL)
//!
//! Both share the same extraction logic since the dependency declaration patterns
//! overlap significantly. The key difference is quoting style:
//! - Groovy: `implementation 'group:artifact:version'`
//! - Kotlin: `implementation("group:artifact:version")`

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `build.gradle` (Groovy DSL) files.
pub struct GradleParser;

/// Parser for `build.gradle.kts` (Kotlin DSL) files.
pub struct GradleKtsParser;

impl ManifestParser for GradleParser {
    fn filename(&self) -> &'static str {
        "build.gradle"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        parse_gradle(content)
    }
}

impl ManifestParser for GradleKtsParser {
    fn filename(&self) -> &'static str {
        "build.gradle.kts"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        parse_gradle(content)
    }
}

fn parse_gradle(content: &str) -> Result<ParsedManifest, ManifestError> {
    let mut deps = Vec::new();
    let mut in_deps_block = false;
    let mut brace_depth: i32 = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        // Enter dependencies { ... } block
        if !in_deps_block
            && (trimmed == "dependencies {"
                || trimmed == "dependencies{"
                || trimmed.starts_with("dependencies {")
                || trimmed.starts_with("dependencies{"))
        {
            in_deps_block = true;
            brace_depth = 1;
            continue;
        }

        if in_deps_block {
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => brace_depth -= 1,
                    _ => {}
                }
            }

            if brace_depth <= 0 {
                in_deps_block = false;
                continue;
            }

            if let Some(dep) = parse_gradle_dep_line(trimmed) {
                deps.push(dep);
            }
        }
    }

    Ok(ParsedManifest {
        ecosystem: "gradle",
        name: None,
        version: None,
        dependencies: deps,
    })
}

/// Configuration names that map to dependency kinds.
///
/// See: https://docs.gradle.org/current/userguide/java_library_plugin.html#sec:java_library_configurations_graph
fn config_kind(config: &str) -> Option<DepKind> {
    let config = config.trim_end_matches('(');
    match config {
        "implementation" | "api" | "compileOnly" | "runtimeOnly" | "compile" | "runtime" => {
            Some(DepKind::Normal)
        }
        "testImplementation"
        | "testCompileOnly"
        | "testRuntimeOnly"
        | "testCompile"
        | "testRuntime"
        | "androidTestImplementation" => Some(DepKind::Dev),
        _ if config.ends_with("TestImplementation")
            || config.ends_with("TestCompile")
            || config.starts_with("debug")
            || config.starts_with("release") =>
        {
            Some(DepKind::Normal)
        }
        _ => None,
    }
}

fn parse_gradle_dep_line(line: &str) -> Option<DeclaredDep> {
    // Find first word (the configuration name)
    let word_end = line.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let config = &line[..word_end];
    let kind = config_kind(config)?;

    let rest = line[word_end..].trim();

    // Extract the coord string — could be:
    //   "group:artifact:version"     (Kotlin/Groovy double-quoted)
    //   'group:artifact:version'     (Groovy single-quoted)
    //   group("artifact") ...        (Kotlin type-safe accessors — skip, no version)
    let coord = if let Some(inner) = rest.strip_prefix('"') {
        let end = inner.find('"')?;
        &inner[..end]
    } else if let Some(inner) = rest.strip_prefix('\'') {
        let end = inner.find('\'')?;
        &inner[..end]
    } else if let Some(inner) = rest.strip_prefix('(') {
        // Kotlin: implementation("...")
        let inner = inner.trim_start();
        if let Some(inner2) = inner.strip_prefix('"') {
            let end = inner2.find('"')?;
            &inner2[..end]
        } else if let Some(inner2) = inner.strip_prefix('\'') {
            let end = inner2.find('\'')?;
            &inner2[..end]
        } else {
            return None;
        }
    } else {
        return None;
    };

    // Parse `group:artifact:version` — also handle `group:artifact` (no version)
    let parts: Vec<&str> = coord.splitn(3, ':').collect();
    match parts.as_slice() {
        [group, artifact, version] => Some(DeclaredDep {
            name: format!("{}:{}", group, artifact),
            version_req: Some(version.to_string()),
            kind,
        }),
        [group, artifact] => Some(DeclaredDep {
            name: format!("{}:{}", group, artifact),
            version_req: None,
            kind,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_build_gradle_groovy() {
        let content = r#"
plugins {
    id 'java'
}

dependencies {
    implementation 'com.google.guava:guava:32.1.0-jre'
    implementation 'org.springframework:spring-core:6.0.0'
    testImplementation 'junit:junit:4.13.2'
    compileOnly 'org.projectlombok:lombok:1.18.28'
}
"#;
        let m = GradleParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "gradle");
        assert_eq!(m.dependencies.len(), 4);

        let guava = m
            .dependencies
            .iter()
            .find(|d| d.name == "com.google.guava:guava")
            .unwrap();
        assert_eq!(guava.version_req.as_deref(), Some("32.1.0-jre"));
        assert_eq!(guava.kind, DepKind::Normal);

        let junit = m
            .dependencies
            .iter()
            .find(|d| d.name == "junit:junit")
            .unwrap();
        assert_eq!(junit.kind, DepKind::Dev);
    }

    #[test]
    fn test_parse_build_gradle_kts() {
        let content = r#"
dependencies {
    implementation("com.google.guava:guava:32.1.0-jre")
    testImplementation("org.junit.jupiter:junit-jupiter:5.9.3")
    api("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.1")
}
"#;
        let m = GradleKtsParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "gradle");
        assert_eq!(m.dependencies.len(), 3);

        let coroutines = m
            .dependencies
            .iter()
            .find(|d| d.name.contains("kotlinx-coroutines-core"))
            .unwrap();
        assert_eq!(coroutines.version_req.as_deref(), Some("1.7.1"));
    }
}
