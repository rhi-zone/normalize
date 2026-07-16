//! TypeScript fact extraction.
//!
//! Recognizes:
//! - `type X = <union of string literals>` → [`Fact::EnumDef`]
//! - `interface X { ... }` property signatures → [`Fact::EntityField`]
//!   (with single-file alias resolution: a property typed as a
//!   `type_identifier` that refers to a local `type X = ...` union-of-string-literals
//!   alias resolves to `TypeShape::Enum`, matching the OVERVIEW.md motivating
//!   example of `status: LessonStatus`)
//! - `function X(a: T1, b: T2): T3 { ... }` → [`Fact::FunctionSignature`]
//! - `const X = v.object({...})` / `const X = z.object({...})` (valibot /
//!   zod validator schemas) → [`Fact::EntityField`] per field, same shape as
//!   `interface` extraction. See the `validators` section below.
//!
//! Node kinds below were confirmed against the real grammar via
//! `normalize syntax ast --compact --depth=-1` on hand-written samples, per
//! CLAUDE.md's "verify before asserting" rule — not guessed from memory.
//!
//! ## Validator schema extraction
//!
//! Recognizes two import styles, tracked per-file in an
//! `identifier -> ImportBinding` map built from top-level `import_statement`
//! nodes:
//! - **Namespace**: `import * as v from "valibot"` — `v` becomes a
//!   [`ImportBinding::Namespace`]; calls take the form `v.object({...})`.
//! - **Named**: `import { object, string } from "valibot"` (optionally
//!   aliased with `as`) — each local identifier becomes an
//!   [`ImportBinding::Named`]; calls are bare, `object({...})`.
//!
//! Recognized validator function names are looked up in a per-library
//! mapping table ([`VALIBOT_FNS`], [`ZOD_FNS`]) that lowers each call to a
//! [`ValidatorKind`], which in turn lowers to a [`TypeShape`] — this mirrors
//! how `lower_type`/`canonical_primitive` map TypeScript's own type syntax.
//!
//! If a file has no recognized valibot/zod import at all, namespace-style
//! calls (`<ns>.object({ field: <ns>.<fn>() })`) are still matched against
//! the mapping tables as a shape-based fallback — the call shape (a chain of
//! member-expression calls using only known validator function names) is
//! distinctive enough to have low false-positive risk, and this catches
//! re-exports under a different alias. The fallback does not extend to bare
//! named-import-style calls (`object({...})` with no `v.`/`z.` prefix),
//! since a bare `object(...)` call is far too generic a shape to guess at
//! without an import to anchor it.

use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use crate::extract::{FactExtractor, FactOccurrence};
use crate::ir::{
    EntityField, EnumDef, Fact, FunctionSignature, NameConfig, TypeShape, canonical_name,
};

/// TypeScript fact extractor.
pub struct TypeScriptExtractor;

impl FactExtractor for TypeScriptExtractor {
    fn grammar_name(&self) -> &'static str {
        "typescript"
    }

    fn extract(
        &self,
        tree: &Tree,
        source: &str,
        file: &str,
        config: &NameConfig,
    ) -> Vec<FactOccurrence> {
        let root = tree.root_node();

        // First pass: collect local type aliases so field types that
        // reference them (`status: LessonStatus`) can resolve to the
        // alias's shape. This is single-file resolution only — no
        // cross-file alias following, per OVERVIEW.md's tractable/hard
        // split.
        let mut aliases: HashMap<String, TypeShape> = HashMap::new();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let child = unwrap_export(child);
            if child.kind() == "type_alias_declaration"
                && let Some(name_node) = child.child_by_field_name("name")
                && let Some(value_node) = child.child_by_field_name("value")
            {
                let name = canonical_name(node_text(name_node, source), config);
                aliases.insert(name, lower_type(value_node, source, config));
            }
        }

        // Second pass: collect validator library import bindings (valibot /
        // zod). `use_fallback` is true only when the file has no recognized
        // import at all — see the module-level doc comment.
        let bindings = collect_import_bindings(root, source);
        let use_fallback = bindings.is_empty();

        let mut out = Vec::new();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let child = unwrap_export(child);
            match child.kind() {
                "type_alias_declaration" => {
                    if let Some(fact) = extract_enum_def(child, source, config) {
                        out.push(occurrence(fact, file, child));
                    }
                }
                "interface_declaration" => {
                    extract_interface(child, source, file, &aliases, config, &mut out);
                }
                "function_declaration" => {
                    if let Some(fact) = extract_function(child, source, config) {
                        out.push(occurrence(fact, file, child));
                    }
                }
                "lexical_declaration" => {
                    extract_validator_schema(
                        child,
                        source,
                        file,
                        &bindings,
                        use_fallback,
                        config,
                        &mut out,
                    );
                }
                _ => {}
            }
        }
        out
    }
}

/// Unwraps a top-level `export_statement` to the declaration node it wraps
/// (`export interface Foo {}` -> the `interface_declaration`, `export const
/// X = ...` -> the `lexical_declaration`, `export type X = ...` -> the
/// `type_alias_declaration`, `export function f() {}` / `export default
/// function f() {}` -> the `function_declaration`), so the top-level
/// dispatch loops can match on declaration kind without caring whether the
/// declaration was exported. Almost all real-world top-level declarations
/// are exported, so skipping this unwrap silently drops nearly everything —
/// confirmed against real code, not a hypothetical: see the `busiless`
/// verification run in the task that added this fix. Non-`export_statement`
/// nodes, and `export default <expr>;` bare-expression exports (no
/// `declaration` field to unwrap to), pass through unchanged.
fn unwrap_export(node: Node) -> Node {
    if node.kind() == "export_statement" {
        node.child_by_field_name("declaration").unwrap_or(node)
    } else {
        node
    }
}

fn occurrence(fact: Fact, file: &str, node: Node) -> FactOccurrence {
    FactOccurrence {
        fact,
        file: file.to_string(),
        line: node.start_position().row + 1,
    }
}

fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

/// If `type_alias_declaration`'s value is a union of string-literal types,
/// lower it to an [`Fact::EnumDef`]. Other alias shapes (object types,
/// generics, ...) are out of scope for this prototype and simply produce no
/// fact — `Fact::EnumDef` extraction is deliberately narrow, not a stub.
fn extract_enum_def(node: Node, source: &str, config: &NameConfig) -> Option<Fact> {
    let name_node = node.child_by_field_name("name")?;
    let value_node = node.child_by_field_name("value")?;
    let mut variants = string_union_variants(value_node, source)?;
    variants.sort();
    variants.dedup();
    Some(Fact::EnumDef(EnumDef {
        name: canonical_name(node_text(name_node, source), config),
        variants,
    }))
}

/// Collects the string-literal variants of a (possibly nested) `union_type`
/// of `literal_type` string literals. Returns `None` if any member of the
/// union isn't a string literal — a mixed union isn't an enum we can
/// represent with this IR yet.
fn string_union_variants(node: Node, source: &str) -> Option<Vec<String>> {
    let mut members = Vec::new();
    collect_union_members(node, &mut members);
    let mut variants = Vec::with_capacity(members.len());
    for member in members {
        variants.push(string_literal_type_value(member, source)?);
    }
    Some(variants.into_iter().map(|s| s.to_string()).collect())
}

/// Flattens a left-recursive `union_type` tree into its leaf members.
fn collect_union_members<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    if node.kind() == "union_type" {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect_union_members(child, out);
        }
    } else {
        out.push(node);
    }
}

/// Extracts the literal string value from a `literal_type` wrapping a
/// `string` node, e.g. `"scheduled"` → `scheduled`.
fn string_literal_type_value<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    if node.kind() != "literal_type" {
        return None;
    }
    string_literal_value(node.named_child(0)?, source)
}

/// Extracts the literal string value from a bare `string` node (as opposed
/// to [`string_literal_type_value`], which unwraps a `literal_type` first —
/// used in type position vs. expression position respectively), e.g.
/// `"scheduled"` → `scheduled`.
fn string_literal_value<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    if node.kind() != "string" {
        return None;
    }
    let fragment = node
        .named_child(0)
        .filter(|n| n.kind() == "string_fragment")?;
    Some(node_text(fragment, source))
}

fn extract_interface(
    node: Node,
    source: &str,
    file: &str,
    aliases: &HashMap<String, TypeShape>,
    config: &NameConfig,
    out: &mut Vec<FactOccurrence>,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let entity = canonical_name(node_text(name_node, source), config);

    let mut cursor = body.walk();
    for member in body.children(&mut cursor) {
        if member.kind() != "property_signature" {
            continue;
        }
        let Some(field_name_node) = member.child_by_field_name("name") else {
            continue;
        };
        let Some(type_annotation) = member.child_by_field_name("type") else {
            continue;
        };
        let Some(type_node) = type_annotation.named_child(0) else {
            continue;
        };

        let optional = has_child_of_kind(member, "?");
        let mut ty = lower_type_resolved(type_node, source, aliases, config);
        if optional {
            ty = TypeShape::Optional(Box::new(ty));
        }

        out.push(occurrence(
            Fact::EntityField(EntityField {
                entity: entity.clone(),
                field: canonical_name(node_text(field_name_node, source), config),
                ty,
            }),
            file,
            member,
        ));
    }
}

fn has_child_of_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| c.kind() == kind)
}

/// Lowers a type node to a [`TypeShape`], resolving `type_identifier`
/// references against locally-declared aliases where possible.
fn lower_type_resolved(
    node: Node,
    source: &str,
    aliases: &HashMap<String, TypeShape>,
    config: &NameConfig,
) -> TypeShape {
    if node.kind() == "type_identifier" {
        let name = canonical_name(node_text(node, source), config);
        if let Some(resolved) = aliases.get(&name) {
            return resolved.clone();
        }
    }
    lower_type(node, source, config)
}

/// Lowers a type node to a [`TypeShape`] without alias resolution.
fn lower_type(node: Node, source: &str, config: &NameConfig) -> TypeShape {
    match node.kind() {
        "predefined_type" => TypeShape::Named(canonical_primitive(node_text(node, source))),
        "type_identifier" => TypeShape::Named(canonical_name(node_text(node, source), config)),
        "array_type" => match node.named_child(0) {
            Some(elem) => TypeShape::Array(Box::new(lower_type(elem, source, config))),
            None => TypeShape::Named(canonical_name(node_text(node, source), config)),
        },
        "union_type" => {
            if let Some(variants) = string_union_variants(node, source) {
                TypeShape::enum_of(variants)
            } else {
                // Mixed/non-literal union: not representable yet, fall back
                // to raw text rather than fabricating structure.
                TypeShape::Named(node_text(node, source).to_string())
            }
        }
        "literal_type" => match string_literal_type_value(node, source) {
            Some(value) => TypeShape::enum_of([value]),
            None => TypeShape::Named(node_text(node, source).to_string()),
        },
        _ => TypeShape::Named(canonical_name(node_text(node, source), config)),
    }
}

/// Canonicalizes a TypeScript primitive keyword (already lowercase in the
/// grammar, e.g. `string`, `number`, `boolean`) into the IR's shared
/// vocabulary. TypeScript's primitive names already match the vocabulary
/// SQL's extractor maps into, so this is currently just a pass-through —
/// kept as its own function so the two extractors document their mapping
/// symmetrically.
fn canonical_primitive(text: &str) -> String {
    text.trim().to_lowercase()
}

fn extract_function(node: Node, source: &str, config: &NameConfig) -> Option<Fact> {
    let name_node = node.child_by_field_name("name")?;
    let parameters = node.child_by_field_name("parameters")?;
    let return_type_node = node
        .child_by_field_name("return_type")
        .and_then(|ann| ann.named_child(0));

    let mut params = Vec::new();
    let mut cursor = parameters.walk();
    for param in parameters.children(&mut cursor) {
        if param.kind() != "required_parameter" && param.kind() != "optional_parameter" {
            continue;
        }
        let Some(pattern) = param.child_by_field_name("pattern") else {
            continue;
        };
        let param_type = param
            .child_by_field_name("type")
            .and_then(|ann| ann.named_child(0))
            .map(|t| lower_type(t, source, config))
            .unwrap_or_else(|| TypeShape::Named("unknown".to_string()));
        let mut param_type = param_type;
        if param.kind() == "optional_parameter" {
            param_type = TypeShape::Optional(Box::new(param_type));
        }
        params.push((
            canonical_name(node_text(pattern, source), config),
            param_type,
        ));
    }

    let returns = return_type_node
        .map(|t| lower_type(t, source, config))
        .unwrap_or_else(|| TypeShape::Named("void".to_string()));

    Some(Fact::FunctionSignature(FunctionSignature {
        name: canonical_name(node_text(name_node, source), config),
        params,
        returns,
    }))
}

// --- Validator schema extraction (valibot / zod) -------------------------

/// Which validator library a local identifier's import binding refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Library {
    Valibot,
    Zod,
}

/// How a local identifier is bound to a validator library import, collected
/// by [`collect_import_bindings`].
#[derive(Debug, Clone)]
enum ImportBinding {
    /// `import * as v from "valibot"` — `v` is the namespace object; calls
    /// take the form `v.<fn>(...)`.
    Namespace(Library),
    /// `import { object } from "valibot"` (optionally aliased with `as`) —
    /// the local identifier directly names the imported function, called
    /// bare: `object(...)`. The `String` is the *imported* (un-aliased)
    /// function name, since that's what indexes into the mapping table.
    Named(Library, String),
}

/// How a validator function call lowers to a [`TypeShape`]. See
/// [`VALIBOT_FNS`] / [`ZOD_FNS`] for the per-library name -> kind tables.
#[derive(Debug, Clone, Copy)]
enum ValidatorKind {
    /// A primitive: lowers to `TypeShape::Named(<the &'static str>)`.
    Named(&'static str),
    /// A closed set of string variants, e.g. `v.picklist([...])` /
    /// `z.enum([...])`: lowers to `TypeShape::Enum`.
    Enum,
    /// A nullable/optional wrapper, recursing on the first argument:
    /// `v.optional(X)` / `v.nullable(X)` both lower the same way (the IR has
    /// one nullable wrapper, `TypeShape::Optional`, for both concepts — see
    /// `ir.rs`'s doc comment on why SQL nullability and TS `?` converge).
    Optional,
    /// `v.array(X)`: lowers to `TypeShape::Array`, recursing on the first
    /// argument.
    Array,
    /// `v.object({...})`: lowers to `TypeShape::Record`, recursing on the
    /// nested object literal's fields.
    Object,
}

/// Valibot's function-name -> [`ValidatorKind`] mapping table.
const VALIBOT_FNS: &[(&str, ValidatorKind)] = &[
    ("string", ValidatorKind::Named("string")),
    ("number", ValidatorKind::Named("number")),
    ("boolean", ValidatorKind::Named("boolean")),
    ("picklist", ValidatorKind::Enum),
    ("optional", ValidatorKind::Optional),
    ("nullable", ValidatorKind::Optional),
    ("array", ValidatorKind::Array),
    ("object", ValidatorKind::Object),
];

/// Zod's function-name -> [`ValidatorKind`] mapping table. Identical to
/// [`VALIBOT_FNS`] except for the enum constructor's name (`enum` vs.
/// `picklist`).
const ZOD_FNS: &[(&str, ValidatorKind)] = &[
    ("string", ValidatorKind::Named("string")),
    ("number", ValidatorKind::Named("number")),
    ("boolean", ValidatorKind::Named("boolean")),
    ("enum", ValidatorKind::Enum),
    ("optional", ValidatorKind::Optional),
    ("nullable", ValidatorKind::Optional),
    ("array", ValidatorKind::Array),
    ("object", ValidatorKind::Object),
];

fn validator_kind(library: Library, fn_name: &str) -> Option<ValidatorKind> {
    let table = match library {
        Library::Valibot => VALIBOT_FNS,
        Library::Zod => ZOD_FNS,
    };
    table
        .iter()
        .find(|(name, _)| *name == fn_name)
        .map(|(_, kind)| *kind)
}

/// Merged fallback table used for shape-based matching when no valibot/zod
/// import was detected in the file (see module-level doc comment). Valibot
/// and zod's function names agree except for the enum constructor
/// (`picklist` vs `enum`); trying both tables lets the fallback recognize
/// either spelling without knowing which library it is.
fn validator_kind_fallback(fn_name: &str) -> Option<ValidatorKind> {
    validator_kind(Library::Valibot, fn_name).or_else(|| validator_kind(Library::Zod, fn_name))
}

/// Maps an import specifier string (e.g. `"valibot"`) to the [`Library`] it
/// names, or `None` for any other module.
fn library_for_module(specifier: &str) -> Option<Library> {
    match specifier {
        "valibot" => Some(Library::Valibot),
        "zod" => Some(Library::Zod),
        _ => None,
    }
}

/// Scans top-level `import_statement` nodes for valibot/zod imports and
/// builds a `local identifier -> ImportBinding` map. Both namespace
/// (`import * as v from "valibot"`) and named (`import { object } from
/// "valibot"`, optionally aliased) styles are recognized; see the
/// module-level doc comment.
fn collect_import_bindings(root: Node, source: &str) -> HashMap<String, ImportBinding> {
    let mut bindings = HashMap::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "import_statement" {
            continue;
        }
        let Some(source_node) = child.child_by_field_name("source") else {
            continue;
        };
        let Some(specifier) = string_literal_value(source_node, source) else {
            continue;
        };
        let Some(library) = library_for_module(specifier) else {
            continue;
        };

        let mut clause_cursor = child.walk();
        let Some(import_clause) = child
            .children(&mut clause_cursor)
            .find(|c| c.kind() == "import_clause")
        else {
            continue;
        };
        let Some(inner) = import_clause.named_child(0) else {
            continue;
        };

        match inner.kind() {
            "namespace_import" => {
                if let Some(ident) = inner.named_child(0) {
                    bindings.insert(
                        node_text(ident, source).to_string(),
                        ImportBinding::Namespace(library),
                    );
                }
            }
            "named_imports" => {
                let mut spec_cursor = inner.walk();
                for spec in inner.named_children(&mut spec_cursor) {
                    if spec.kind() != "import_specifier" {
                        continue;
                    }
                    let Some(name_node) = spec.child_by_field_name("name") else {
                        continue;
                    };
                    let imported = node_text(name_node, source).to_string();
                    let local = spec
                        .child_by_field_name("alias")
                        .map(|a| node_text(a, source).to_string())
                        .unwrap_or_else(|| imported.clone());
                    bindings.insert(local, ImportBinding::Named(library, imported));
                }
            }
            _ => {}
        }
    }
    bindings
}

/// Resolves a `call_expression` node to the [`ValidatorKind`] it invokes,
/// under `bindings`. Handles both call shapes:
/// - Namespace-style (`v.object(...)`): the function is a `member_expression`
///   whose object is a bound namespace identifier (or, when `use_fallback`
///   is set, any identifier — see the module-level doc comment).
/// - Named-import-style (`object(...)`): the function is a bare `identifier`
///   bound to `ImportBinding::Named`. No fallback: too generic a shape to
///   guess at without an import to anchor it.
///
/// Returns the resolved kind together with the call node itself, so the
/// caller can pull `arguments` back off it.
fn resolve_validator_call<'a>(
    node: Node<'a>,
    source: &str,
    bindings: &HashMap<String, ImportBinding>,
    use_fallback: bool,
) -> Option<(ValidatorKind, Node<'a>)> {
    if node.kind() != "call_expression" {
        return None;
    }
    let function = node.child_by_field_name("function")?;
    match function.kind() {
        "member_expression" => {
            let object = function.child_by_field_name("object")?;
            let property = function.child_by_field_name("property")?;
            if object.kind() != "identifier" || property.kind() != "property_identifier" {
                return None;
            }
            let object_name = node_text(object, source);
            let fn_name = node_text(property, source);
            let kind = match bindings.get(object_name) {
                Some(ImportBinding::Namespace(library)) => validator_kind(*library, fn_name),
                _ if use_fallback => validator_kind_fallback(fn_name),
                _ => None,
            }?;
            Some((kind, node))
        }
        "identifier" => {
            let local_name = node_text(function, source);
            let ImportBinding::Named(library, imported_name) = bindings.get(local_name)? else {
                return None;
            };
            let kind = validator_kind(*library, imported_name)?;
            Some((kind, node))
        }
        _ => None,
    }
}

/// Lowers a validator call expression (e.g. `v.optional(v.string())`) to a
/// [`TypeShape`], recursing into nested calls for `Optional`/`Array`/`Object`
/// kinds. Returns `None` if the node isn't a recognized validator call.
fn lower_validator_value(
    node: Node,
    source: &str,
    bindings: &HashMap<String, ImportBinding>,
    use_fallback: bool,
    config: &NameConfig,
) -> Option<TypeShape> {
    let (kind, call_node) = resolve_validator_call(node, source, bindings, use_fallback)?;
    let arguments = call_node.child_by_field_name("arguments")?;
    let mut arg_cursor = arguments.walk();
    let first_arg = arguments.named_children(&mut arg_cursor).next();

    match kind {
        ValidatorKind::Named(name) => Some(TypeShape::Named(name.to_string())),
        ValidatorKind::Enum => {
            let arg = first_arg?;
            if arg.kind() != "array" {
                return None;
            }
            let mut cursor = arg.walk();
            let variants: Vec<String> = arg
                .named_children(&mut cursor)
                .filter_map(|c| string_literal_value(c, source))
                .map(|s| s.to_string())
                .collect();
            if variants.is_empty() {
                None
            } else {
                Some(TypeShape::enum_of(variants))
            }
        }
        ValidatorKind::Optional => {
            let inner = lower_validator_value(first_arg?, source, bindings, use_fallback, config)?;
            Some(TypeShape::Optional(Box::new(inner)))
        }
        ValidatorKind::Array => {
            let inner = lower_validator_value(first_arg?, source, bindings, use_fallback, config)?;
            Some(TypeShape::Array(Box::new(inner)))
        }
        ValidatorKind::Object => {
            let fields = object_literal_fields(first_arg?, source, bindings, use_fallback, config);
            if fields.is_empty() {
                None
            } else {
                Some(TypeShape::record_of(
                    fields.into_iter().map(|(f, t, _)| (f, t)).collect(),
                ))
            }
        }
    }
}

/// Extracts the plain text of an object-literal key node: either a bare
/// `property_identifier` (`name: ...`) or a quoted `string` (`"name":
/// ...`) — the latter is common in generated code (see e.g. `busiless`'s
/// `*.schema.generated.ts` codegen output) and must canonicalize to the
/// same field name as the bare spelling, not to a field literally named
/// `"name"` with quote characters baked in.
fn object_key_text<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    match node.kind() {
        "property_identifier" => Some(node_text(node, source)),
        "string" => string_literal_value(node, source),
        _ => None,
    }
}

/// Extracts `(field name, type, 1-based line)` triples from an `object`
/// literal node's `pair` children, lowering each value via
/// [`lower_validator_value`]. A pair whose value doesn't lower to a
/// recognized validator call is skipped, not fabricated.
fn object_literal_fields(
    object_node: Node,
    source: &str,
    bindings: &HashMap<String, ImportBinding>,
    use_fallback: bool,
    config: &NameConfig,
) -> Vec<(String, TypeShape, usize)> {
    if object_node.kind() != "object" {
        return Vec::new();
    }
    let mut cursor = object_node.walk();
    let mut fields = Vec::new();
    for pair in object_node.named_children(&mut cursor) {
        if pair.kind() != "pair" {
            continue;
        }
        let Some(key_node) = pair.child_by_field_name("key") else {
            continue;
        };
        let Some(key_text) = object_key_text(key_node, source) else {
            continue;
        };
        let Some(value_node) = pair.child_by_field_name("value") else {
            continue;
        };
        let Some(ty) = lower_validator_value(value_node, source, bindings, use_fallback, config)
        else {
            continue;
        };
        fields.push((
            canonical_name(key_text, config),
            ty,
            pair.start_position().row + 1,
        ));
    }
    fields
}

/// Recognizes top-level `const X = <validator>.object({...})` declarations
/// and emits one [`Fact::EntityField`] per recognized field, mirroring
/// [`extract_interface`]'s shape so a validator schema and a same-shape
/// `interface` converge (once [`NameConfig`] strips the `Schema`/etc. suffix
/// off the variable name).
fn extract_validator_schema(
    node: Node,
    source: &str,
    file: &str,
    bindings: &HashMap<String, ImportBinding>,
    use_fallback: bool,
    config: &NameConfig,
    out: &mut Vec<FactOccurrence>,
) {
    let mut cursor = node.walk();
    for declarator in node.children(&mut cursor) {
        if declarator.kind() != "variable_declarator" {
            continue;
        }
        let Some(name_node) = declarator.child_by_field_name("name") else {
            continue;
        };
        let Some(value_node) = declarator.child_by_field_name("value") else {
            continue;
        };
        let Some((kind, call_node)) =
            resolve_validator_call(value_node, source, bindings, use_fallback)
        else {
            continue;
        };
        if !matches!(kind, ValidatorKind::Object) {
            continue;
        }
        let Some(arguments) = call_node.child_by_field_name("arguments") else {
            continue;
        };
        let mut arg_cursor = arguments.walk();
        let Some(obj_arg) = arguments.named_children(&mut arg_cursor).next() else {
            continue;
        };

        let entity = canonical_name(node_text(name_node, source), config);
        let fields = object_literal_fields(obj_arg, source, bindings, use_fallback, config);
        for (field, ty, line) in fields {
            out.push(FactOccurrence {
                fact: Fact::EntityField(EntityField {
                    entity: entity.clone(),
                    field,
                    ty,
                }),
                file: file.to_string(),
                line,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract_from_source;
    use crate::restatement::{find_restatements, find_similar, restated_only};

    fn extract(source: &str) -> Vec<Fact> {
        extract_from_source(
            &TypeScriptExtractor,
            source,
            "src/schema.ts",
            &NameConfig::default(),
        )
        .expect("typescript grammar should load and parse")
        .into_iter()
        .map(|o| o.fact)
        .collect()
    }

    #[test]
    fn valibot_namespace_object_extracts_entity_fields() {
        const SRC: &str = r#"
import * as v from "valibot";

const NpsSurveyRowSchema = v.object({
  id: v.string(),
  status: v.picklist(["scheduled", "in_progress", "completed", "cancelled"]),
  title: v.optional(v.string()),
  tags: v.array(v.string()),
  score: v.nullable(v.number()),
});
"#;
        let facts = extract(SRC);

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "id".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "status".to_string(),
            ty: TypeShape::enum_of(["scheduled", "in_progress", "completed", "cancelled"]),
        })));
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "title".to_string(),
            ty: TypeShape::Optional(Box::new(TypeShape::Named("string".to_string()))),
        })));
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "tags".to_string(),
            ty: TypeShape::Array(Box::new(TypeShape::Named("string".to_string()))),
        })));
        // `nullable` lowers the same way as `optional` — see `ValidatorKind::Optional`.
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "score".to_string(),
            ty: TypeShape::Optional(Box::new(TypeShape::Named("number".to_string()))),
        })));
    }

    #[test]
    fn zod_namespace_object_extracts_entity_fields() {
        const SRC: &str = r#"
import * as z from "zod";

const NpsSurveyRowSchema = z.object({
  id: z.string(),
  status: z.enum(["scheduled", "in_progress"]),
  title: z.optional(z.string()),
});
"#;
        let facts = extract(SRC);

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "id".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "status".to_string(),
            ty: TypeShape::enum_of(["scheduled", "in_progress"]),
        })));
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "title".to_string(),
            ty: TypeShape::Optional(Box::new(TypeShape::Named("string".to_string()))),
        })));
    }

    /// The core convergence thesis for this task: a TS `interface` and a
    /// same-shape valibot schema, once the schema's `Schema` suffix is
    /// stripped by `NameConfig`, lower to byte-identical fact IR nodes and
    /// group together under both exact and approximate restatement finding.
    #[test]
    fn interface_and_validator_schema_converge_under_suffix_stripping() {
        const SRC: &str = r#"
interface NpsSurveyRow {
  id: string;
  status: "scheduled" | "in_progress" | "completed" | "cancelled";
}

import * as v from "valibot";

const NpsSurveyRowSchema = v.object({
  id: v.string(),
  status: v.picklist(["scheduled", "in_progress", "completed", "cancelled"]),
});
"#;
        let occurrences = extract_from_source(
            &TypeScriptExtractor,
            SRC,
            "src/schema.ts",
            &NameConfig::default(),
        )
        .expect("typescript grammar should load and parse");

        let restated = restated_only(find_restatements(&occurrences));
        let id_group = restated
            .iter()
            .find(|g| {
                matches!(&g.fact, Fact::EntityField(f) if f.entity == "nps survey row" && f.field == "id")
            })
            .expect("interface and schema `id` fields should converge under exact matching");
        assert_eq!(id_group.count(), 2);

        let status_group = restated
            .iter()
            .find(|g| {
                matches!(&g.fact, Fact::EntityField(f) if f.entity == "nps survey row" && f.field == "status")
            })
            .expect("interface and schema `status` fields should converge under exact matching");
        assert_eq!(status_group.count(), 2);

        // Approximate mode groups by identity (entity+field) regardless of
        // exact type match; since the types are already exactly equal here,
        // it should show one group with a single distinct shape and 2 total
        // occurrences — convergence under approximate matching too.
        let similar = find_similar(&occurrences);
        let id_similar = similar
            .iter()
            .find(|g| {
                g.entries.iter().any(|e| {
                    matches!(&e.fact, Fact::EntityField(f) if f.entity == "nps survey row" && f.field == "id")
                })
            })
            .expect("id should be grouped by approximate mode too");
        assert_eq!(
            id_similar.entries.len(),
            1,
            "types match exactly, one shape"
        );
        assert_eq!(id_similar.total_count(), 2);
    }

    #[test]
    fn collect_import_bindings_distinguishes_namespace_and_named_styles() {
        const SRC: &str = r#"
import * as v from "valibot";
import { object, string, picklist as pl } from "zod";
"#;
        let tree = normalize_languages::parsers::parse_with_grammar("typescript", SRC)
            .expect("typescript grammar should load and parse");
        let bindings = collect_import_bindings(tree.root_node(), SRC);

        assert!(matches!(
            bindings.get("v"),
            Some(ImportBinding::Namespace(Library::Valibot))
        ));
        assert!(matches!(
            bindings.get("object"),
            Some(ImportBinding::Named(Library::Zod, name)) if name == "object"
        ));
        assert!(matches!(
            bindings.get("string"),
            Some(ImportBinding::Named(Library::Zod, name)) if name == "string"
        ));
        // Aliased named import: local name is the alias, but the mapped
        // function name is the *imported* (un-aliased) one.
        assert!(matches!(
            bindings.get("pl"),
            Some(ImportBinding::Named(Library::Zod, name)) if name == "picklist"
        ));
        assert_eq!(bindings.len(), 4);
    }

    /// With no valibot/zod import in the file at all, the namespace-style
    /// call shape (`<ns>.object({ field: <ns>.<fn>() })`) is still
    /// recognized as a shape-based fallback — this is what catches
    /// re-exports under a different alias.
    #[test]
    fn shape_based_fallback_matches_without_validator_import() {
        const SRC: &str = r#"
import { myHelper } from "./local-utils";

const NpsSurveyRowSchema = v.object({
  id: v.string(),
  status: v.picklist(["scheduled", "in_progress"]),
});
"#;
        let facts = extract(SRC);

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "id".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "nps survey row".to_string(),
            field: "status".to_string(),
            ty: TypeShape::enum_of(["scheduled", "in_progress"]),
        })));
    }

    /// Named-import-style bare calls (`object({...})`) are *not* matched by
    /// the shape-based fallback — too generic a call shape to guess at
    /// without an import binding to anchor it.
    #[test]
    fn shape_based_fallback_does_not_apply_to_bare_named_import_style() {
        const SRC: &str = r#"
const NpsSurveyRowSchema = object({
  id: string(),
});
"#;
        let facts = extract(SRC);
        assert!(
            facts.is_empty(),
            "bare named-import-style calls with no import binding should not be guessed at: {facts:#?}"
        );
    }

    #[test]
    fn custom_suffix_list_changes_entity_canonicalization() {
        const SRC: &str = r#"
import * as v from "valibot";

const NpsSurveyRowValidator = v.object({
  id: v.string(),
});
"#;
        // Default config strips "Validator", so the entity name is "nps
        // survey row".
        let default_facts = extract(SRC);
        assert!(default_facts.iter().any(|f| {
            matches!(f, Fact::EntityField(field) if field.entity == "nps survey row")
        }));

        // A custom config that doesn't include "Validator" in its suffix
        // list should leave it in place instead.
        let custom_config = NameConfig {
            strip_suffixes: vec!["Schema".to_string()],
        };
        let occurrences =
            extract_from_source(&TypeScriptExtractor, SRC, "src/schema.ts", &custom_config)
                .expect("typescript grammar should load and parse");
        let custom_facts: Vec<Fact> = occurrences.into_iter().map(|o| o.fact).collect();
        assert!(custom_facts.iter().any(|f| {
            matches!(f, Fact::EntityField(field) if field.entity == "nps survey row validator")
        }));
    }

    /// Regression test for `unwrap_export`: real-world TypeScript almost
    /// always exports its top-level declarations, and prior to this fix the
    /// extractor's top-level dispatch loops matched directly on
    /// `export_statement`'s child kind — which is always `export_statement`
    /// itself for an exported declaration, never `interface_declaration` /
    /// `type_alias_declaration` / `lexical_declaration` /
    /// `function_declaration` — so every exported declaration was silently
    /// skipped. Discovered while verifying convergence against a real
    /// repository (busiless): a valibot schema and its corresponding
    /// `export interface` produced zero facts each, not just zero
    /// convergence.
    #[test]
    fn exported_declarations_are_extracted() {
        const SRC: &str = r#"
import * as v from "valibot";

export interface Foo {
  id: string;
}

export type Bar = "a" | "b";

export function baz(a: string): boolean {
  return true;
}

export const FooSchema = v.object({
  id: v.string(),
});
"#;
        let facts = extract(SRC);

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "foo".to_string(),
            field: "id".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));
        assert!(facts.contains(&Fact::EnumDef(EnumDef {
            name: "bar".to_string(),
            variants: vec!["a".to_string(), "b".to_string()],
        })));
        assert!(facts.contains(&Fact::FunctionSignature(FunctionSignature {
            name: "baz".to_string(),
            params: vec![("a".to_string(), TypeShape::Named("string".to_string()))],
            returns: TypeShape::Named("boolean".to_string()),
        })));
        // The validator schema entity name canonicalizes to the same "foo"
        // as the interface (default config strips the "Schema" suffix),
        // converging both under exact IR equality.
        assert_eq!(
            facts
                .iter()
                .filter(|f| matches!(f, Fact::EntityField(field) if field.entity == "foo" && field.field == "id" && field.ty == TypeShape::Named("string".to_string())))
                .count(),
            2,
            "exported interface and exported validator schema should both extract and converge: {facts:#?}"
        );
    }

    /// Regression test for quoted object-literal keys: generated schema code
    /// (`busiless`'s codegen output uses this style) often writes
    /// `"fieldName": v.string()` with a quoted key rather than a bare
    /// identifier. Both spellings must canonicalize to the same field name
    /// — not to a field literally named `"fieldName"` with quote characters
    /// baked in, which would silently prevent convergence with the bare
    /// spelling used elsewhere.
    #[test]
    fn quoted_object_keys_canonicalize_the_same_as_bare_identifiers() {
        const SRC: &str = r#"
import * as v from "valibot";

const FooSchema = v.object({
  "name": v.string(),
});
"#;
        let facts = extract(SRC);
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "foo".to_string(),
            field: "name".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));
    }
}
