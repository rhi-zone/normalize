//! `normalize config` — Schema-driven config inspection and validation.
//!
//! Generic engine: works with any TOML/JSON/YAML file + any JSON Schema.
//! Defaults to `.normalize/config.toml` + `NormalizeConfig` schema.

use crate::config::NormalizeConfig;
use crate::output::OutputFormatter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::path::{Path, PathBuf};

// ── Report types ────────────────────────────────────────────────────────────

/// Report from `normalize config show`: current config values with schema annotations.
///
/// `content` holds the parsed config file values; `schema` (skipped from JSON output)
/// is used to render human-readable annotations alongside each field. `set_only` limits
/// output to fields explicitly set in the file (skipping defaults).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConfigShowReport {
    pub config_path: String,
    pub section: Option<String>,
    /// Current config values (the file content).
    pub content: serde_json::Value,
    /// If true, only show fields that have a value set in the config file.
    #[serde(skip)]
    #[schemars(skip)]
    pub set_only: bool,
    /// Full JSON Schema — skipped from JSON output, used for annotated rendering.
    #[serde(skip)]
    #[schemars(skip)]
    pub schema: serde_json::Value,
}

impl OutputFormatter for ConfigShowReport {
    fn format_text(&self) -> String {
        let section_path: Vec<String> = self
            .section
            .as_deref()
            .map(parse_section_path)
            .unwrap_or_default();
        let section_path_refs: Vec<&str> = section_path.iter().map(String::as_str).collect();

        let mut out = format!("# Config: {}", self.config_path);
        if let Some(ref sec) = self.section {
            out.push_str(&format!(" — [{sec}]"));
        }
        out.push_str("\n\n");

        // Navigate schema and content to the requested path
        let section_schema = navigate_schema(&self.schema, &section_path_refs);
        let section_content = navigate_json(&self.content, &section_path_refs);

        out.push_str(&format_schema_annotated(
            &self.schema,
            section_schema,
            section_content,
            &section_path_refs,
            self.set_only,
        ));
        out.trim_end().to_string()
    }
}

/// Parse a dotted section path, handling quoted segments like `rules."rust/unwrap-in-impl"`.
/// `rules."foo/bar"` → `["rules", "foo/bar"]`; `a.b.c` → `["a", "b", "c"]`.
fn parse_section_path(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in s.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            '.' if !in_quote => {
                if !cur.is_empty() {
                    parts.push(cur.clone());
                    cur.clear();
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

/// Navigate a JSON value along a dotted key path.
fn navigate_json<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut cur = value;
    for key in path {
        cur = cur.get(key)?;
    }
    Some(cur)
}

/// Resolve a `$ref` in the schema (local `#/$defs/...` refs only).
fn resolve_ref<'a>(
    root: &'a serde_json::Value,
    schema: &'a serde_json::Value,
) -> &'a serde_json::Value {
    if let Some(r) = schema.get("$ref").and_then(|v| v.as_str())
        && let Some(path) = r.strip_prefix("#/")
    {
        let mut cur = root;
        for part in path.split('/') {
            match cur.get(part) {
                Some(v) => cur = v,
                None => return schema,
            }
        }
        return cur;
    }
    schema
}

/// Navigate the schema along a key path, resolving `$ref` at each step.
/// Falls back to `additionalProperties` when a key isn't in `properties`.
///
/// `root` is always the top-level schema (for `$ref` resolution);
/// `current` is the schema node at the current path position.
fn navigate_schema_inner<'a>(
    root: &'a serde_json::Value,
    current: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    if path.is_empty() {
        return Some(current);
    }
    let child = current
        .get("properties")
        .and_then(|p| p.get(path[0]))
        .or_else(|| current.get("additionalProperties"))?;
    // Always resolve $refs against the root schema, not the current node
    let resolved = resolve_ref(root, child);
    navigate_schema_inner(root, resolved, &path[1..])
}

fn navigate_schema<'a>(
    root: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    navigate_schema_inner(root, root, path)
}

/// Format the type annotation for a schema node, including array item types.
fn format_type(schema: &serde_json::Value) -> String {
    let raw_type = schema.get("type");

    // Unwrap nullable types: ["string", "null"] → "string (optional)"
    let base = if let Some(serde_json::Value::Array(types)) = raw_type {
        let non_null: Vec<&str> = types
            .iter()
            .filter_map(|t| t.as_str())
            .filter(|t| *t != "null")
            .collect();
        let nullable = types.iter().any(|t| t.as_str() == Some("null"));
        let base = non_null.join(" | ");
        if nullable {
            format!("{base} (optional)")
        } else {
            base
        }
    } else {
        raw_type
            .and_then(|t| t.as_str())
            .unwrap_or("any")
            .to_string()
    };

    // For arrays, append item type
    if base.starts_with("array")
        && let Some(items) = schema.get("items")
    {
        let item_type = items.get("type").and_then(|t| t.as_str()).unwrap_or("any");
        return format!("array of {item_type}");
    }

    base
}

/// Format a single JSON value as a TOML-compatible inline string for a comment.
fn json_val_inline(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => format!("{s:?}"),
        serde_json::Value::Null => "(unset)".to_string(),
        other => other.to_string(),
    }
}

/// Render a schema section with current values annotated by schema descriptions.
/// If `set_only` is true, unset fields are omitted; otherwise all schema properties are shown.
fn format_schema_annotated(
    root_schema: &serde_json::Value,
    section_schema: Option<&serde_json::Value>,
    section_content: Option<&serde_json::Value>,
    section_path: &[&str],
    set_only: bool,
) -> String {
    let Some(schema) = section_schema else {
        // No schema for this path — fall back to raw content as TOML
        return match (section_path.last(), section_content) {
            (Some(key), Some(v)) => json_to_toml_string(&serde_json::json!({ *key: v })),
            (_, Some(v)) => json_to_toml_string(v),
            _ => "(not found)".to_string(),
        };
    };

    let resolved = resolve_ref(root_schema, schema);

    // Leaf node (not an object with properties) — show its schema details
    let Some(props) = resolved.get("properties").and_then(|p| p.as_object()) else {
        let mut out = String::new();
        // Description
        if let Some(desc) = resolved.get("description").and_then(|d| d.as_str()) {
            for line in desc.lines() {
                out.push_str(&format!("# {line}\n"));
            }
        }
        // Type info
        if let Some(t) = resolved.get("type") {
            let type_str = format_type(resolved);
            out.push_str(&format!("# type: {type_str}\n"));
            let _ = t; // used via format_type
        }
        // Current value or default
        let key = section_path.last().copied().unwrap_or("value");
        match section_content {
            Some(v) if !v.is_null() => {
                let toml_str = json_to_toml_string(&serde_json::json!({ key: v }));
                out.push_str(toml_str.trim_end());
            }
            _ => {
                let default_str = resolved
                    .get("default")
                    .map(json_val_inline)
                    .unwrap_or_else(|| "(unset)".to_string());
                out.push_str(&format!("# {key} = {default_str}"));
            }
        }
        return out;
    };

    // Object with properties — show section description then all fields
    let mut out = String::new();
    if let Some(desc) = resolved.get("description").and_then(|d| d.as_str()) {
        for line in desc.lines() {
            out.push_str(&format!("# {line}\n"));
        }
        out.push('\n');
    }

    for (key, prop_schema) in props {
        let prop_resolved = resolve_ref(root_schema, prop_schema);
        let current = section_content.and_then(|c| c.get(key));
        let is_set = current.map(|v| !v.is_null()).unwrap_or(false);

        // Skip unset fields when set_only is active
        if set_only && !is_set {
            continue;
        }

        // Buffer this field so we can append it atomically
        let mut field_buf = String::new();

        // Description
        if let Some(desc) = prop_resolved.get("description").and_then(|d| d.as_str()) {
            for line in desc.lines() {
                field_buf.push_str(&format!("# {line}\n"));
            }
        }

        if is_set {
            let v = current.unwrap();
            // Render as TOML key = value
            let toml_str = json_to_toml_string(&serde_json::json!({ key: v }));
            field_buf.push_str(toml_str.trim_end());
        } else {
            // Show default or (unset) as a comment
            let default_str = prop_schema
                .get("default")
                .map(json_val_inline)
                .unwrap_or_else(|| "(unset)".to_string());
            field_buf.push_str(&format!("# {key} = {default_str}"));
        }
        field_buf.push_str("\n\n");
        out.push_str(&field_buf);
    }

    // If the schema has additionalProperties, also enumerate content keys not in `properties`.
    // This covers dynamic keys like per-rule overrides in [rules."rule-id"] sections.
    if let (Some(add_schema), Some(content_obj)) = (
        resolved.get("additionalProperties"),
        section_content.and_then(|c| c.as_object()),
    ) {
        let schema_keys: std::collections::HashSet<&str> =
            props.keys().map(String::as_str).collect();
        let _add_resolved = resolve_ref(root_schema, add_schema);

        // Show a header if there are any dynamic entries
        let dynamic_keys: Vec<(&str, &serde_json::Value)> = content_obj
            .iter()
            .filter(|(k, _)| !schema_keys.contains(k.as_str()))
            .map(|(k, v)| (k.as_str(), v))
            .collect();

        if !dynamic_keys.is_empty() {
            // Build the parent path string for section headers
            let parent_path = section_path.join(".");

            for (key, value) in dynamic_keys {
                let quoted_key = if key.contains('/') || key.contains('-') || key.contains('.') {
                    format!("\"{key}\"")
                } else {
                    key.to_string()
                };
                let section_header = if parent_path.is_empty() {
                    quoted_key.clone()
                } else {
                    format!("{parent_path}.{quoted_key}")
                };
                out.push_str(&format!("[{section_header}]\n"));
                let toml_str = json_to_toml_string(value);
                out.push_str(toml_str.trim_end());
                out.push_str("\n\n");
            }
        }
    }

    out
}

/// Report from `normalize config validate`: whether the config file is schema-compliant.
///
/// `valid` is true only when `errors` is empty. `schema_source` identifies which schema
/// was used for validation (e.g. the crate's built-in schema or a custom path).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConfigValidateReport {
    pub valid: bool,
    pub errors: Vec<String>,
    pub config_path: String,
    pub schema_source: String,
}

impl OutputFormatter for ConfigValidateReport {
    fn format_text(&self) -> String {
        if self.valid {
            format!("✓ Config is valid: {}", self.config_path)
        } else {
            let mut out = format!(
                "✗ Config is invalid: {} ({} error(s))\n",
                self.config_path,
                self.errors.len()
            );
            for err in &self.errors {
                out.push_str(&format!("  - {err}\n"));
            }
            out.trim_end().to_string()
        }
    }
}

/// Report from `normalize config set`: records the key, old value, and new value applied.
///
/// `dry_run` is true when the change was previewed but not written. `schema_warnings`
/// lists any schema violations that were present but bypassed via `--force` or `--dry-run`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConfigSetReport {
    pub key: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
    pub config_path: String,
    pub dry_run: bool,
    /// Schema errors that were present but bypassed (via --force or --dry-run).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub schema_warnings: Vec<String>,
}

impl OutputFormatter for ConfigSetReport {
    fn format_text(&self) -> String {
        let prefix = if self.dry_run { "[dry-run] " } else { "" };
        let old = self
            .old_value
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(unset)".to_string());
        let mut out = format!(
            "{prefix}Set {key}: {old} → {new}\n  in {path}",
            key = self.key,
            new = self.new_value,
            path = self.config_path,
        );
        if !self.schema_warnings.is_empty() {
            let label = if self.dry_run {
                "Schema errors (would require --force to write):"
            } else {
                "Schema errors (written with --force):"
            };
            out.push_str(&format!("\n\n{label}"));
            for w in &self.schema_warnings {
                out.push_str(&format!("\n  - {w}"));
            }
        }
        out
    }
}

// ── Private helpers ─────────────────────────────────────────────────────────

fn default_config_file(root: &Path) -> String {
    root.join(".normalize")
        .join("config.toml")
        .to_string_lossy()
        .into_owned()
}

fn load_schema(schema_path: Option<&str>) -> Result<serde_json::Value, String> {
    match schema_path {
        Some(p) => {
            let raw =
                std::fs::read_to_string(p).map_err(|e| format!("Cannot read schema '{p}': {e}"))?;
            serde_json::from_str(&raw).map_err(|e| format!("Invalid JSON Schema: {e}"))
        }
        None => {
            let schema = schemars::schema_for!(NormalizeConfig);
            serde_json::to_value(schema).map_err(|e| format!("Schema serialization error: {e}"))
        }
    }
}

fn load_file_as_json(file_path: &str) -> Result<serde_json::Value, String> {
    let raw = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Cannot read '{file_path}': {e}"))?;
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "toml" => {
            let v: toml::Value = toml::from_str(&raw)
                .map_err(|e| format!("TOML parse error in '{file_path}': {e}"))?;
            serde_json::to_value(v).map_err(|e| format!("TOML→JSON conversion error: {e}"))
        }
        "json" => serde_json::from_str(&raw)
            .map_err(|e| format!("JSON parse error in '{file_path}': {e}")),
        "yaml" | "yml" => serde_yaml::from_str(&raw)
            .map_err(|e| format!("YAML parse error in '{file_path}': {e}")),
        _ => Err(format!(
            "Unsupported file format: .{ext} (supported: toml, json, yaml)"
        )),
    }
}

fn validate_against_schema(
    schema_json: &serde_json::Value,
    instance: &serde_json::Value,
) -> Vec<String> {
    match jsonschema::validator_for(schema_json) {
        Err(e) => vec![format!("Schema compile error: {e}")],
        Ok(validator) => validator
            .iter_errors(instance)
            .map(|e| e.to_string())
            .collect(),
    }
}

/// Convert a `serde_json::Value` to a pretty TOML string, falling back to JSON.
fn json_to_toml_string(value: &serde_json::Value) -> String {
    // serde_json → toml::Value → toml string
    match serde_json::from_value::<toml::Value>(value.clone()) {
        Ok(tv) => toml::to_string_pretty(&tv).unwrap_or_else(|_| value.to_string()),
        Err(_) => value.to_string(),
    }
}

/// Type-coerce a string value for TOML: bool → i64 → f64 → string.
fn coerce_value(s: &str) -> toml_edit::Value {
    if s == "true" {
        return toml_edit::Value::from(true);
    }
    if s == "false" {
        return toml_edit::Value::from(false);
    }
    if let Ok(n) = s.parse::<i64>() {
        return toml_edit::Value::from(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return toml_edit::Value::from(f);
    }
    toml_edit::Value::from(s)
}

/// Navigate (or create) a chain of TOML tables and set the leaf value.
fn set_dotted_key(
    doc: &mut toml_edit::DocumentMut,
    keys: &[&str],
    value: toml_edit::Value,
) -> Result<Option<toml_edit::Value>, String> {
    if keys.is_empty() {
        return Err("Key must not be empty".to_string());
    }
    if keys.len() == 1 {
        let key = keys[0];
        let old = doc.get(key).and_then(|i| i.as_value().cloned());
        doc[key] = toml_edit::value(value);
        return Ok(old);
    }

    // Navigate into nested tables
    let (head, tail) = keys.split_first().unwrap();
    if !doc.contains_key(head) {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        doc[*head] = toml_edit::Item::Table(t);
    }
    let table = doc[*head]
        .as_table_mut()
        .ok_or_else(|| format!("'{head}' is not a table"))?;
    set_in_table(table, tail, value)
}

fn set_in_table(
    table: &mut toml_edit::Table,
    keys: &[&str],
    value: toml_edit::Value,
) -> Result<Option<toml_edit::Value>, String> {
    if keys.len() == 1 {
        let key = keys[0];
        let old = table.get(key).and_then(|i| i.as_value().cloned());
        table[key] = toml_edit::value(value);
        return Ok(old);
    }
    let (head, tail) = keys.split_first().unwrap();
    if !table.contains_key(head) {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        table[*head] = toml_edit::Item::Table(t);
    }
    let child = table[*head]
        .as_table_mut()
        .ok_or_else(|| format!("'{head}' is not a table"))?;
    set_in_table(child, tail, value)
}

// ── Service ─────────────────────────────────────────────────────────────────

/// Config inspection and mutation service for `normalize config` subcommands.
///
/// Wraps `show`, `validate`, `set`, `get`, and `edit` operations against
/// `.normalize/config.toml`. Schema-driven: uses the `NormalizeConfig` JSON Schema
/// to annotate output and validate writes.
pub struct ConfigService {
    pub(crate) pretty: Cell<bool>,
}

impl ConfigService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        // Share parent pretty cell value but own our Cell
        Self {
            pretty: Cell::new(pretty.get()),
        }
    }

    fn display_show(&self, r: &ConfigShowReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_validate(&self, r: &ConfigValidateReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn display_set(&self, r: &ConfigSetReport) -> String {
        if self.pretty.get() {
            r.format_pretty()
        } else {
            r.format_text()
        }
    }

    fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
        root.map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)
            .map_err(|e| format!("Failed to get current directory: {e}"))
    }

    fn resolve_format(&self, pretty: bool, compact: bool, root: &Path) {
        self.pretty
            .set(super::resolve_pretty(root, pretty, compact));
    }
}

#[server_less::cli(
    name = "config",
    description = "Inspect and validate config files using JSON Schema",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl ConfigService {
    /// Emit the JSON Schema for .normalize/config.toml (NormalizeConfig)
    ///
    /// Examples:
    ///   normalize config schema              # print the full JSON Schema
    ///   normalize config schema --json       # machine-readable JSON output
    pub fn schema(&self, pretty: bool, compact: bool) -> Result<serde_json::Value, String> {
        let root = std::env::current_dir().unwrap_or_default();
        self.resolve_format(pretty, compact, &root);
        let schema = schemars::schema_for!(NormalizeConfig);
        serde_json::to_value(schema).map_err(|e| format!("Schema serialization error: {e}"))
    }

    /// Show a config file with schema annotations — all available options, with descriptions.
    /// Use --section for a dotted path (e.g. 'analyze', 'analyze.threshold').
    /// Use --set-only to hide fields that have no value set in the config file.
    ///
    /// Examples:
    ///   normalize config show                                  # show all options with descriptions
    ///   normalize config show --set-only                       # only show fields with values set
    ///   normalize config show --section rules                  # show the \[rules\] section
    ///   normalize config show --section rules."rust/unwrap-in-impl"  # show a specific rule config
    #[allow(clippy::too_many_arguments)]
    #[cli(display_with = "display_show")]
    pub fn show(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Path to config file (default: .normalize/config.toml)")] file: Option<
            String,
        >,
        #[param(help = "Path to JSON Schema file (default: NormalizeConfig schema)")]
        schema: Option<String>,
        #[param(
            help = "Show a specific section or field (dotted path, e.g. 'analyze', 'analyze.threshold')"
        )]
        section: Option<String>,
        #[param(help = "Only show fields that have a value set in the config file")] set_only: bool,
        pretty: bool,
        compact: bool,
    ) -> Result<ConfigShowReport, String> {
        let root_path = Self::resolve_root(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let config_path = file.unwrap_or_else(|| default_config_file(&root_path));
        let schema_json = load_schema(schema.as_deref())?;
        // Config file is optional — show schema even if no file exists yet
        let content = load_file_as_json(&config_path)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        Ok(ConfigShowReport {
            config_path,
            section,
            set_only,
            content,
            schema: schema_json,
        })
    }

    /// Validate a config file against its JSON Schema
    ///
    /// Examples:
    ///   normalize config validate                        # validate .normalize/config.toml
    ///   normalize config validate --file custom.toml     # validate a custom config file
    #[cli(display_with = "display_validate")]
    pub fn validate(
        &self,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Path to config file (default: .normalize/config.toml)")] file: Option<
            String,
        >,
        #[param(help = "Path to JSON Schema file (default: NormalizeConfig schema)")]
        schema: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<ConfigValidateReport, String> {
        let root_path = Self::resolve_root(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let config_path = file.unwrap_or_else(|| default_config_file(&root_path));
        let schema_source = schema
            .clone()
            .unwrap_or_else(|| "NormalizeConfig (built-in)".to_string());
        let schema_json = load_schema(schema.as_deref())?;
        let instance = load_file_as_json(&config_path)?;
        let errors = validate_against_schema(&schema_json, &instance);
        let valid = errors.is_empty();
        if valid {
            Ok(ConfigValidateReport {
                valid,
                errors,
                config_path,
                schema_source,
            })
        } else {
            // Return Ok so server-less displays the report; exit code handled by valid field
            Err(ConfigValidateReport {
                valid,
                errors,
                config_path,
                schema_source,
            }
            .format_text())
        }
    }

    /// Set a config value by dotted key path (TOML files only)
    ///
    /// Examples:
    ///   normalize config set analyze.threshold 50                                    # set a numeric value
    ///   normalize config set analyze.rules."rust/unwrap-in-impl".severity warning    # set rule severity
    ///   normalize config set analyze.rules."rust/unwrap-in-impl".enabled false       # disable a rule
    ///   normalize config set --dry-run analyze.threshold 50                          # preview without writing
    ///   normalize config set --force analyze.custom-key value                        # bypass schema validation
    #[cli(display_with = "display_set")]
    #[allow(clippy::too_many_arguments)]
    pub fn set(
        &self,
        #[param(positional, help = "Dotted key path (e.g. 'analyze.clones')")] key: String,
        #[param(positional, help = "New value (bool/int/float/string auto-detected)")]
        value: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Path to config file (default: .normalize/config.toml)")] file: Option<
            String,
        >,
        #[param(help = "Path to JSON Schema file (default: NormalizeConfig schema)")]
        schema: Option<String>,
        #[param(help = "Preview changes without writing")] dry_run: bool,
        #[param(help = "Write even if the value fails schema validation")] force: bool,
        pretty: bool,
        compact: bool,
    ) -> Result<ConfigSetReport, String> {
        let root_path = Self::resolve_root(root)?;
        self.resolve_format(pretty, compact, &root_path);
        let config_path = file.unwrap_or_else(|| default_config_file(&root_path));

        // Only TOML supported for writes
        let ext = Path::new(&config_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if ext != "toml" {
            return Err(format!(
                "`config set` only supports TOML files; got .{ext} in '{config_path}'"
            ));
        }

        // Load existing content (create empty doc if file doesn't exist)
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = content
            .parse()
            .map_err(|e| format!("TOML parse error in '{config_path}': {e}"))?;

        let keys: Vec<&str> = key.split('.').collect();
        let coerced = coerce_value(&value);
        let old_value = set_dotted_key(&mut doc, &keys, coerced)?;

        // Validate post-mutation
        let schema_json = load_schema(schema.as_deref())?;
        let new_doc_str = doc.to_string();
        let new_instance: serde_json::Value = {
            let tv: toml::Value = toml::from_str(&new_doc_str)
                .map_err(|e| format!("Post-set TOML parse error: {e}"))?;
            serde_json::to_value(tv).map_err(|e| format!("Post-set conversion error: {e}"))?
        };
        let schema_errors = validate_against_schema(&schema_json, &new_instance);

        // Block on schema errors unless --force or --dry-run
        if !schema_errors.is_empty() && !force && !dry_run {
            let mut msg = format!(
                "Schema validation failed for '{key}':\n{}",
                schema_errors
                    .iter()
                    .map(|e| format!("  - {e}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            msg.push_str("\nRun with --force to write anyway.");
            return Err(msg);
        }

        // Read back the new value for reporting
        let new_value: serde_json::Value = {
            let tv: toml::Value =
                toml::from_str(&new_doc_str).unwrap_or(toml::Value::String(value.clone()));
            // Navigate to the key
            let mut v = serde_json::to_value(&tv).unwrap_or(serde_json::Value::Null);
            for k in &keys {
                v = v.get(*k).cloned().unwrap_or(serde_json::Value::Null);
            }
            v
        };

        let old_json = old_value.map(|tv| {
            serde_json::to_value(
                toml::Value::try_from(tv.to_string()).unwrap_or(toml::Value::String(String::new())),
            )
            .unwrap_or(serde_json::Value::Null)
        });

        if !dry_run {
            std::fs::write(&config_path, &new_doc_str)
                .map_err(|e| format!("Cannot write '{config_path}': {e}"))?;
        }

        Ok(ConfigSetReport {
            key,
            old_value: old_json,
            new_value,
            config_path,
            dry_run,
            schema_warnings: schema_errors,
        })
    }
}
