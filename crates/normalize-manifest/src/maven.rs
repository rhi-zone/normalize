//! Parser for `pom.xml` files (Java/Maven).
//!
//! Extracts `<dependency>` elements from the `<dependencies>` section.
//! Supports `<scope>` → DepKind mapping:
//! - `test` → Dev
//! - `provided` / `optional` → Optional
//! - `compile` / `runtime` / (absent) → Normal

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `pom.xml` files.
pub struct MavenParser;

impl ManifestParser for MavenParser {
    fn filename(&self) -> &'static str {
        "pom.xml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let doc = roxmltree::Document::parse(content).map_err(|e| ManifestError(e.to_string()))?;

        let root = doc.root_element();

        // Extract project name and version from top-level elements (not parent's)
        let name = root
            .children()
            .find(|n| n.has_tag_name("artifactId"))
            .and_then(|n| n.text())
            .map(|s| s.to_string());

        let version = root
            .children()
            .find(|n| n.has_tag_name("version"))
            .and_then(|n| n.text())
            .map(|s| s.to_string());

        let mut deps = Vec::new();

        // Walk all <dependency> nodes anywhere in the doc (handles <dependencies> inside
        // <dependencyManagement> as well, though those are typically constraints not deps)
        for dep_node in doc.descendants().filter(|n| n.has_tag_name("dependency")) {
            // Skip nodes inside <dependencyManagement> — they're version constraints, not direct deps
            if dep_node
                .ancestors()
                .any(|a| a.has_tag_name("dependencyManagement"))
            {
                continue;
            }

            let group_id = child_text(&dep_node, "groupId").unwrap_or_default();
            let artifact_id = child_text(&dep_node, "artifactId").unwrap_or_default();
            let version_str = child_text(&dep_node, "version");
            let scope = child_text(&dep_node, "scope");
            let optional_flag = child_text(&dep_node, "optional");

            if group_id.is_empty() || artifact_id.is_empty() {
                continue;
            }

            let kind = if optional_flag.as_deref() == Some("true") {
                DepKind::Optional
            } else {
                match scope.as_deref() {
                    Some("test") => DepKind::Dev,
                    Some("provided") => DepKind::Optional,
                    _ => DepKind::Normal,
                }
            };

            // Strip property placeholders like ${project.version} — keep as-is but recognizable
            let version_req = version_str.map(|v| v.to_string());

            deps.push(DeclaredDep {
                name: format!("{}:{}", group_id, artifact_id),
                version_req,
                kind,
            });
        }

        Ok(ParsedManifest {
            ecosystem: "maven",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn child_text<'a>(node: &roxmltree::Node<'a, '_>, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_pom_xml() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>my-app</artifactId>
  <version>1.0.0</version>

  <dependencies>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-web</artifactId>
      <version>3.1.0</version>
    </dependency>
    <dependency>
      <groupId>com.fasterxml.jackson.core</groupId>
      <artifactId>jackson-databind</artifactId>
      <version>2.15.0</version>
    </dependency>
    <dependency>
      <groupId>org.junit.jupiter</groupId>
      <artifactId>junit-jupiter</artifactId>
      <version>5.9.3</version>
      <scope>test</scope>
    </dependency>
    <dependency>
      <groupId>javax.servlet</groupId>
      <artifactId>servlet-api</artifactId>
      <version>2.5</version>
      <scope>provided</scope>
    </dependency>
  </dependencies>
</project>"#;

        let m = MavenParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "maven");
        assert_eq!(m.name.as_deref(), Some("my-app"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));
        assert_eq!(m.dependencies.len(), 4);

        let spring = m
            .dependencies
            .iter()
            .find(|d| d.name.contains("spring-boot-starter-web"))
            .unwrap();
        assert_eq!(spring.kind, DepKind::Normal);
        assert_eq!(spring.version_req.as_deref(), Some("3.1.0"));

        let junit = m
            .dependencies
            .iter()
            .find(|d| d.name.contains("junit-jupiter"))
            .unwrap();
        assert_eq!(junit.kind, DepKind::Dev);

        let servlet = m
            .dependencies
            .iter()
            .find(|d| d.name.contains("servlet-api"))
            .unwrap();
        assert_eq!(servlet.kind, DepKind::Optional);
    }

    #[test]
    fn test_pom_xml_skips_dependency_management() {
        let content = r#"<?xml version="1.0"?>
<project>
  <artifactId>bom-project</artifactId>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>org.example</groupId>
        <artifactId>managed-dep</artifactId>
        <version>2.0</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
  <dependencies>
    <dependency>
      <groupId>org.example</groupId>
      <artifactId>real-dep</artifactId>
      <version>1.0</version>
    </dependency>
  </dependencies>
</project>"#;
        let m = MavenParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "org.example:real-dep");
    }
}
