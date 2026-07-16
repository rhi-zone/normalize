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
//!
//! Node kinds below were confirmed against the real grammar via
//! `normalize syntax ast --compact --depth=-1` on hand-written samples, per
//! CLAUDE.md's "verify before asserting" rule — not guessed from memory.

use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use crate::extract::{FactExtractor, FactOccurrence};
use crate::ir::{EntityField, EnumDef, Fact, FunctionSignature, TypeShape, canonical_name};

/// TypeScript fact extractor.
pub struct TypeScriptExtractor;

impl FactExtractor for TypeScriptExtractor {
    fn grammar_name(&self) -> &'static str {
        "typescript"
    }

    fn extract(&self, tree: &Tree, source: &str, file: &str) -> Vec<FactOccurrence> {
        let root = tree.root_node();

        // First pass: collect local type aliases so field types that
        // reference them (`status: LessonStatus`) can resolve to the
        // alias's shape. This is single-file resolution only — no
        // cross-file alias following, per OVERVIEW.md's tractable/hard
        // split.
        let mut aliases: HashMap<String, TypeShape> = HashMap::new();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "type_alias_declaration"
                && let Some(name_node) = child.child_by_field_name("name")
                && let Some(value_node) = child.child_by_field_name("value")
            {
                let name = canonical_name(node_text(name_node, source));
                aliases.insert(name, lower_type(value_node, source));
            }
        }

        let mut out = Vec::new();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "type_alias_declaration" => {
                    if let Some(fact) = extract_enum_def(child, source) {
                        out.push(occurrence(fact, file, child));
                    }
                }
                "interface_declaration" => {
                    extract_interface(child, source, file, &aliases, &mut out);
                }
                "function_declaration" => {
                    if let Some(fact) = extract_function(child, source) {
                        out.push(occurrence(fact, file, child));
                    }
                }
                _ => {}
            }
        }
        out
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
fn extract_enum_def(node: Node, source: &str) -> Option<Fact> {
    let name_node = node.child_by_field_name("name")?;
    let value_node = node.child_by_field_name("value")?;
    let mut variants = string_union_variants(value_node, source)?;
    variants.sort();
    variants.dedup();
    Some(Fact::EnumDef(EnumDef {
        name: canonical_name(node_text(name_node, source)),
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
    let string_node = node.named_child(0)?;
    if string_node.kind() != "string" {
        return None;
    }
    let fragment = string_node
        .named_child(0)
        .filter(|n| n.kind() == "string_fragment")?;
    Some(node_text(fragment, source))
}

fn extract_interface(
    node: Node,
    source: &str,
    file: &str,
    aliases: &HashMap<String, TypeShape>,
    out: &mut Vec<FactOccurrence>,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let entity = canonical_name(node_text(name_node, source));

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
        let mut ty = lower_type_resolved(type_node, source, aliases);
        if optional {
            ty = TypeShape::Optional(Box::new(ty));
        }

        out.push(occurrence(
            Fact::EntityField(EntityField {
                entity: entity.clone(),
                field: canonical_name(node_text(field_name_node, source)),
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
) -> TypeShape {
    if node.kind() == "type_identifier" {
        let name = canonical_name(node_text(node, source));
        if let Some(resolved) = aliases.get(&name) {
            return resolved.clone();
        }
    }
    lower_type(node, source)
}

/// Lowers a type node to a [`TypeShape`] without alias resolution.
fn lower_type(node: Node, source: &str) -> TypeShape {
    match node.kind() {
        "predefined_type" => TypeShape::Named(canonical_primitive(node_text(node, source))),
        "type_identifier" => TypeShape::Named(canonical_name(node_text(node, source))),
        "array_type" => match node.named_child(0) {
            Some(elem) => TypeShape::Array(Box::new(lower_type(elem, source))),
            None => TypeShape::Named(canonical_name(node_text(node, source))),
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
        _ => TypeShape::Named(canonical_name(node_text(node, source))),
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

fn extract_function(node: Node, source: &str) -> Option<Fact> {
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
            .map(|t| lower_type(t, source))
            .unwrap_or_else(|| TypeShape::Named("unknown".to_string()));
        let mut param_type = param_type;
        if param.kind() == "optional_parameter" {
            param_type = TypeShape::Optional(Box::new(param_type));
        }
        params.push((canonical_name(node_text(pattern, source)), param_type));
    }

    let returns = return_type_node
        .map(|t| lower_type(t, source))
        .unwrap_or_else(|| TypeShape::Named("void".to_string()));

    Some(Fact::FunctionSignature(FunctionSignature {
        name: canonical_name(node_text(name_node, source)),
        params,
        returns,
    }))
}
