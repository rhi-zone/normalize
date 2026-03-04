//! Parsers for NuGet package manifests (.NET).
//!
//! - `packages.config` (legacy NuGet format)
//! - `*.csproj` (SDK-style, `<PackageReference>` elements)

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `packages.config` files (legacy NuGet).
///
/// Format: `<package id="Name" version="1.0" />`
pub struct PackagesConfigParser;

impl ManifestParser for PackagesConfigParser {
    fn filename(&self) -> &'static str {
        "packages.config"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let doc = roxmltree::Document::parse(content).map_err(|e| ManifestError(e.to_string()))?;

        let mut deps = Vec::new();

        for node in doc.descendants().filter(|n| n.has_tag_name("package")) {
            let id = node.attribute("id");
            let version = node.attribute("version");
            let dev_dep = node.attribute("developmentDependency");

            if let Some(name) = id {
                let kind = if dev_dep == Some("true") {
                    DepKind::Dev
                } else {
                    DepKind::Normal
                };
                deps.push(DeclaredDep {
                    name: name.to_string(),
                    version_req: version.map(|v| v.to_string()),
                    kind,
                });
            }
        }

        Ok(ParsedManifest {
            ecosystem: "nuget",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

/// Parser for SDK-style `*.csproj` files (NuGet PackageReference).
///
/// Handles:
/// - `<PackageReference Include="Name" Version="1.0" />`
/// - `<PackageReference Include="Name"><Version>1.0</Version></PackageReference>`
pub struct CsprojParser;

impl CsprojParser {
    /// Parse a `.csproj` file content directly (for extension-based dispatch).
    pub fn parse_content(content: &str) -> Result<ParsedManifest, ManifestError> {
        CsprojParser.parse(content)
    }
}

impl ManifestParser for CsprojParser {
    fn filename(&self) -> &'static str {
        "*.csproj"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let doc = roxmltree::Document::parse(content).map_err(|e| ManifestError(e.to_string()))?;

        // Project name/version from PropertyGroup
        let name = doc
            .descendants()
            .find(|n| n.has_tag_name("AssemblyName"))
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string());

        let version = doc
            .descendants()
            .find(|n| n.has_tag_name("Version"))
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string());

        let mut deps = Vec::new();

        for node in doc
            .descendants()
            .filter(|n| n.has_tag_name("PackageReference"))
        {
            let pkg_name = node
                .attribute("Include")
                .or_else(|| node.attribute("include"));
            if pkg_name.is_none() {
                continue;
            }
            let pkg_name = pkg_name.unwrap().to_string();

            // Version can be attribute or child element
            let version_req = node
                .attribute("Version")
                .or_else(|| node.attribute("version"))
                .map(|v| v.to_string())
                .or_else(|| {
                    node.children()
                        .find(|n| n.has_tag_name("Version"))
                        .and_then(|n| n.text())
                        .map(|v| v.trim().to_string())
                });

            // PrivateAssets="all" typically marks dev/build tools
            let private_assets = node
                .attribute("PrivateAssets")
                .or_else(|| node.attribute("privateAssets"));
            let kind = if private_assets == Some("all") {
                DepKind::Dev
            } else {
                DepKind::Normal
            };

            deps.push(DeclaredDep {
                name: pkg_name,
                version_req,
                kind,
            });
        }

        Ok(ParsedManifest {
            ecosystem: "nuget",
            name,
            version,
            dependencies: deps,
        })
    }
}

/// Parser for `Directory.Packages.props` files (.NET Central Package Management).
///
/// Extracts `<PackageVersion Include="Name" Version="..." />` elements.
/// All entries are `Normal` — this file declares available versions, not scopes.
pub struct DirectoryPackagesPropsParser;

impl ManifestParser for DirectoryPackagesPropsParser {
    fn filename(&self) -> &'static str {
        "Directory.Packages.props"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let doc = roxmltree::Document::parse(content).map_err(|e| ManifestError(e.to_string()))?;

        let mut deps = Vec::new();

        for node in doc
            .descendants()
            .filter(|n| n.has_tag_name("PackageVersion"))
        {
            let pkg_name = node
                .attribute("Include")
                .or_else(|| node.attribute("include"));
            let Some(pkg_name) = pkg_name else {
                continue;
            };

            let version_req = node
                .attribute("Version")
                .or_else(|| node.attribute("version"))
                .map(|v| v.to_string());

            deps.push(DeclaredDep {
                name: pkg_name.to_string(),
                version_req,
                kind: DepKind::Normal,
            });
        }

        Ok(ParsedManifest {
            ecosystem: "nuget",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_packages_config() {
        let content = r#"<?xml version="1.0" encoding="utf-8"?>
<packages>
  <package id="Newtonsoft.Json" version="13.0.3" targetFramework="net48" />
  <package id="NUnit" version="3.13.3" targetFramework="net48" />
  <package id="StyleCop.Analyzers" version="1.1.118" developmentDependency="true" />
</packages>"#;

        let m = PackagesConfigParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "nuget");
        assert_eq!(m.dependencies.len(), 3);

        let json = m
            .dependencies
            .iter()
            .find(|d| d.name == "Newtonsoft.Json")
            .unwrap();
        assert_eq!(json.version_req.as_deref(), Some("13.0.3"));
        assert_eq!(json.kind, DepKind::Normal);

        let style = m
            .dependencies
            .iter()
            .find(|d| d.name == "StyleCop.Analyzers")
            .unwrap();
        assert_eq!(style.kind, DepKind::Dev);
    }

    #[test]
    fn test_csproj_package_reference() {
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <AssemblyName>MyApp</AssemblyName>
    <Version>2.0.0</Version>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="Microsoft.EntityFrameworkCore" Version="8.0.0" />
    <PackageReference Include="coverlet.collector" Version="6.0.0" PrivateAssets="all" />
  </ItemGroup>
</Project>"#;

        let m = CsprojParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "nuget");
        assert_eq!(m.name.as_deref(), Some("MyApp"));
        assert_eq!(m.version.as_deref(), Some("2.0.0"));
        assert_eq!(m.dependencies.len(), 3);

        let efcore = m
            .dependencies
            .iter()
            .find(|d| d.name == "Microsoft.EntityFrameworkCore")
            .unwrap();
        assert_eq!(efcore.version_req.as_deref(), Some("8.0.0"));
        assert_eq!(efcore.kind, DepKind::Normal);

        let coverlet = m
            .dependencies
            .iter()
            .find(|d| d.name == "coverlet.collector")
            .unwrap();
        assert_eq!(coverlet.kind, DepKind::Dev);
    }

    #[test]
    fn test_directory_packages_props() {
        let content = r#"<Project>
  <PropertyGroup>
    <ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally>
  </PropertyGroup>
  <ItemGroup>
    <PackageVersion Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageVersion Include="Microsoft.EntityFrameworkCore" Version="8.0.0" />
    <PackageVersion Include="coverlet.collector" Version="6.0.0" />
  </ItemGroup>
</Project>"#;

        let m = DirectoryPackagesPropsParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "nuget");
        assert_eq!(m.dependencies.len(), 3);

        let json_dep = m
            .dependencies
            .iter()
            .find(|d| d.name == "Newtonsoft.Json")
            .unwrap();
        assert_eq!(json_dep.version_req.as_deref(), Some("13.0.3"));
        assert_eq!(json_dep.kind, DepKind::Normal);

        let efcore = m
            .dependencies
            .iter()
            .find(|d| d.name == "Microsoft.EntityFrameworkCore")
            .unwrap();
        assert_eq!(efcore.version_req.as_deref(), Some("8.0.0"));
        assert_eq!(efcore.kind, DepKind::Normal);

        let coverlet = m
            .dependencies
            .iter()
            .find(|d| d.name == "coverlet.collector")
            .unwrap();
        assert_eq!(coverlet.version_req.as_deref(), Some("6.0.0"));
        assert_eq!(coverlet.kind, DepKind::Normal);
    }
}
