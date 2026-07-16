//! SQL fact extraction.
//!
//! Recognizes `CREATE TABLE` column definitions → [`Fact::EntityField`],
//! including `CHECK (col IN (...))` constraints lowered to
//! `TypeShape::Enum`.
//!
//! No `Fact::FunctionSignature` or `Fact::EnumDef` extraction: this
//! prototype's SQL samples are schema DDL, and SQL has no named top-level
//! enum declaration to lower `EnumDef` from (an inline `CHECK IN` list has
//! no name of its own — it's naturally a `TypeShape::Enum` on the field, not
//! a `Fact::EnumDef`). `CREATE FUNCTION` extraction is a real gap, not a
//! stub: left for a follow-up once the IR's function-signature shape is
//! exercised against more than TypeScript.
//!
//! Node kinds below were confirmed against the real grammar via
//! `normalize syntax ast --compact --depth=-1` on hand-written samples, per
//! CLAUDE.md's "verify before asserting" rule — not guessed from memory.

use tree_sitter::{Node, Tree};

use crate::extract::{FactExtractor, FactOccurrence};
use crate::ir::{EntityField, Fact, TypeShape, canonical_name};

/// SQL fact extractor.
pub struct SqlExtractor;

impl FactExtractor for SqlExtractor {
    fn grammar_name(&self) -> &'static str {
        "sql"
    }

    fn extract(&self, tree: &Tree, source: &str, file: &str) -> Vec<FactOccurrence> {
        let root = tree.root_node();
        let mut out = Vec::new();
        walk_create_tables(root, source, file, &mut out);
        out
    }
}

fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn walk_create_tables(node: Node, source: &str, file: &str, out: &mut Vec<FactOccurrence>) {
    if node.kind() == "create_table" {
        extract_create_table(node, source, file, out);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_create_tables(child, source, file, out);
    }
}

fn extract_create_table(node: Node, source: &str, file: &str, out: &mut Vec<FactOccurrence>) {
    let mut cursor = node.walk();
    let Some(table_ref) = node
        .children(&mut cursor)
        .find(|c| c.kind() == "object_reference")
    else {
        return;
    };
    let Some(name_node) = table_ref.child_by_field_name("name") else {
        return;
    };
    let entity = canonical_name(node_text(name_node, source));

    let mut cursor = node.walk();
    let Some(columns) = node
        .children(&mut cursor)
        .find(|c| c.kind() == "column_definitions")
    else {
        return;
    };

    let mut cursor = columns.walk();
    for column in columns.children(&mut cursor) {
        if column.kind() != "column_definition" {
            continue;
        }
        let Some(field_name_node) = column.child_by_field_name("name") else {
            continue;
        };
        let Some(type_node) = column.child_by_field_name("type") else {
            continue;
        };

        let mut ty = check_in_enum(column, source).unwrap_or_else(|| lower_type(type_node, source));
        if !is_not_null(column) {
            ty = TypeShape::Optional(Box::new(ty));
        }

        out.push(FactOccurrence {
            fact: Fact::EntityField(EntityField {
                entity: entity.clone(),
                field: canonical_name(node_text(field_name_node, source)),
                ty,
            }),
            file: file.to_string(),
            line: column.start_position().row + 1,
        });
    }
}

/// A column is `NOT NULL` if it has adjacent `keyword_not` + `keyword_null`
/// children. Absence means the column is nullable, which the IR represents
/// as `TypeShape::Optional` — the same wrapper TypeScript's `?` produces, so
/// a nullable SQL column and an optional TypeScript property converge.
fn is_not_null(column: Node) -> bool {
    let mut cursor = column.walk();
    let kinds: Vec<&str> = column.children(&mut cursor).map(|c| c.kind()).collect();
    kinds
        .windows(2)
        .any(|w| w == ["keyword_not", "keyword_null"])
}

/// If the column has a `CHECK (col IN (...))` constraint, lower the
/// literal list to `TypeShape::Enum`. This is what lets a SQL `CHECK IN`
/// list converge with a TypeScript string-literal union — both are closed
/// sets of string variants, just spelled differently.
fn check_in_enum(column: Node, source: &str) -> Option<TypeShape> {
    let mut cursor = column.walk();
    let binary_expr = find_descendant(column, &mut cursor, "binary_expression")?;
    let operator = binary_expr.child_by_field_name("operator")?;
    if operator.kind() != "keyword_in" {
        return None;
    }
    let right = binary_expr.child_by_field_name("right")?;
    if right.kind() != "list" {
        return None;
    }
    let mut cursor = right.walk();
    let variants: Vec<String> = right
        .children(&mut cursor)
        .filter(|c| c.kind() == "literal")
        .map(|c| strip_quotes(node_text(c, source)).to_string())
        .collect();
    if variants.is_empty() {
        None
    } else {
        Some(TypeShape::enum_of(variants))
    }
}

fn find_descendant<'a>(
    node: Node<'a>,
    cursor: &mut tree_sitter::TreeCursor<'a>,
    kind: &str,
) -> Option<Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }
    for child in node.children(cursor) {
        let mut child_cursor = child.walk();
        if let Some(found) = find_descendant(child, &mut child_cursor, kind) {
            return Some(found);
        }
    }
    None
}

fn strip_quotes(text: &str) -> &str {
    text.trim_matches(|c| c == '\'' || c == '"')
}

/// Canonicalizes a SQL column type node into the IR's shared vocabulary.
/// SQL's type keywords vary a lot more across dialects than this prototype
/// attempts to handle — only the primitives exercised by the test fixtures
/// are mapped; anything else falls back to its lowercased source text
/// rather than fabricating a mapping.
fn lower_type(node: Node, source: &str) -> TypeShape {
    let name = match node.kind() {
        "keyword_text" | "keyword_varchar" | "keyword_char" => "string".to_string(),
        "int" | "keyword_integer" => "number".to_string(),
        "keyword_boolean" => "boolean".to_string(),
        _ => canonical_name(node_text(node, source)),
    };
    TypeShape::Named(name)
}
