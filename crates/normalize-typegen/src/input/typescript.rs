//! TypeScript type extraction parser.
//!
//! Extracts type definitions from TypeScript source files
//! (interfaces, type aliases, enums) into the typegen IR.

use super::ParseError;
use crate::ir::{
    EnumDef, EnumKind, Field, IntVariant, Schema, StringVariant, StructDef, Type, TypeDef,
    TypeDefKind,
};
use tree_sitter::{Node, Parser, Tree};

/// Parse TypeScript source and extract type definitions into IR.
pub fn parse_typescript_types(source: &str) -> Result<Schema, ParseError> {
    let mut parser = Parser::new();
    parser
        .set_language(&arborium_typescript::language().into())
        .map_err(|e| ParseError::Unsupported(format!("tree-sitter init: {}", e)))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::Unsupported("failed to parse TypeScript".into()))?;

    let ctx = ExtractContext::new(source);
    ctx.extract_schema(&tree)
}

struct ExtractContext<'a> {
    source: &'a str,
}

impl<'a> ExtractContext<'a> {
    fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    fn extract_schema(&self, tree: &Tree) -> Result<Schema, ParseError> {
        let root = tree.root_node();
        let mut schema = Schema::new();
        let mut pending_comment: Option<String> = None;

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "comment" => {
                    pending_comment = self.extract_doc_comment(child);
                }
                "interface_declaration" => {
                    let mut def = self.extract_interface(child)?;
                    if def.docs.is_none() {
                        def.docs = pending_comment.take();
                    }
                    schema.add(def);
                    pending_comment = None;
                }
                "type_alias_declaration" => {
                    let mut def = self.extract_type_alias(child)?;
                    if def.docs.is_none() {
                        def.docs = pending_comment.take();
                    }
                    schema.add(def);
                    pending_comment = None;
                }
                "enum_declaration" => {
                    let mut def = self.extract_enum(child)?;
                    if def.docs.is_none() {
                        def.docs = pending_comment.take();
                    }
                    schema.add(def);
                    pending_comment = None;
                }
                "export_statement" => {
                    if let Some(decl) = child.child_by_field_name("declaration") {
                        match decl.kind() {
                            "interface_declaration" => {
                                let mut def = self.extract_interface(decl)?;
                                if def.docs.is_none() {
                                    def.docs = pending_comment.take();
                                }
                                schema.add(def);
                            }
                            "type_alias_declaration" => {
                                let mut def = self.extract_type_alias(decl)?;
                                if def.docs.is_none() {
                                    def.docs = pending_comment.take();
                                }
                                schema.add(def);
                            }
                            "enum_declaration" => {
                                let mut def = self.extract_enum(decl)?;
                                if def.docs.is_none() {
                                    def.docs = pending_comment.take();
                                }
                                schema.add(def);
                            }
                            _ => {}
                        }
                    }
                    pending_comment = None;
                }
                _ => {
                    pending_comment = None;
                }
            }
        }

        Ok(schema)
    }

    fn extract_doc_comment(&self, node: Node) -> Option<String> {
        let text = self.node_text(node);
        if text.starts_with("/**") {
            // JSDoc comment - strip delimiters and leading asterisks
            let inner = text
                .strip_prefix("/**")
                .and_then(|s| s.strip_suffix("*/"))
                .unwrap_or(text);
            let lines: Vec<&str> = inner
                .lines()
                .map(|line| line.trim().trim_start_matches('*').trim())
                .filter(|line| !line.is_empty())
                .collect();
            if lines.is_empty() {
                None
            } else {
                Some(lines.join(" "))
            }
        } else {
            None
        }
    }

    fn extract_interface(&self, node: Node) -> Result<TypeDef, ParseError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ParseError::Unsupported("interface missing name".into()))?;
        let name_str = self.node_text(name).to_string();

        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ParseError::Unsupported("interface missing body".into()))?;

        let fields = self.extract_interface_body(body)?;

        Ok(TypeDef {
            name: name_str,
            docs: None,
            kind: TypeDefKind::Struct(StructDef { fields }),
        })
    }

    fn extract_interface_body(&self, body: Node) -> Result<Vec<Field>, ParseError> {
        let mut fields = Vec::new();
        let mut pending_comment: Option<String> = None;
        let mut cursor = body.walk();

        for child in body.children(&mut cursor) {
            match child.kind() {
                "comment" => {
                    pending_comment = self.extract_doc_comment(child);
                }
                "property_signature" => {
                    let mut field = self.extract_property_signature(child)?;
                    if field.docs.is_none() {
                        field.docs = pending_comment.take();
                    }
                    fields.push(field);
                    pending_comment = None;
                }
                _ => {
                    pending_comment = None;
                }
            }
        }

        Ok(fields)
    }

    fn extract_property_signature(&self, node: Node) -> Result<Field, ParseError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ParseError::Unsupported("property missing name".into()))?;
        let name_str = self.node_text(name).to_string();

        // Check for optional marker (?)
        let optional = self.has_question_mark(node);

        // Get type annotation
        let ty = if let Some(type_ann) = node.child_by_field_name("type") {
            self.extract_type_from_annotation(type_ann)?
        } else {
            Type::Any
        };

        Ok(Field {
            name: name_str,
            ty,
            required: !optional,
            docs: None,
        })
    }

    fn has_question_mark(&self, node: Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() && self.node_text(child) == "?" {
                return true;
            }
        }
        false
    }

    fn extract_type_from_annotation(&self, node: Node) -> Result<Type, ParseError> {
        // type_annotation has a `:` child then the actual type node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                return self.extract_type(child);
            }
        }
        Ok(Type::Any)
    }

    fn extract_type(&self, node: Node) -> Result<Type, ParseError> {
        match node.kind() {
            "predefined_type" => self.extract_predefined_type(node),

            "type_identifier" => {
                let name = self.node_text(node);
                Ok(Type::Ref(name.to_string()))
            }

            "union_type" => self.extract_union_type(node),

            "array_type" => {
                // T[]
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.is_named() {
                        let inner = self.extract_type(child)?;
                        return Ok(Type::Array(Box::new(inner)));
                    }
                }
                Ok(Type::Array(Box::new(Type::Any)))
            }

            "generic_type" => self.extract_generic_type(node),

            "literal_type" => self.extract_literal_type(node),

            "parenthesized_type" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.is_named() {
                        return self.extract_type(child);
                    }
                }
                Ok(Type::Any)
            }

            "tuple_type" => {
                // Simplified: treat as Array<Any>
                Ok(Type::Array(Box::new(Type::Any)))
            }

            "intersection_type" => {
                // Simplified: return Any (proper handling would merge fields)
                Ok(Type::Any)
            }

            "object_type" => {
                // Inline object type - treat as Any for now
                Ok(Type::Any)
            }

            "function_type" | "constructor_type" => {
                // Function types don't map to the typegen IR
                Ok(Type::Any)
            }

            _ => Ok(Type::Any),
        }
    }

    fn extract_predefined_type(&self, node: Node) -> Result<Type, ParseError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "string" => return Ok(Type::String),
                "number" => return Ok(Type::Float { bits: 64 }),
                "boolean" => return Ok(Type::Boolean),
                "any" | "unknown" => return Ok(Type::Any),
                "void" | "undefined" | "never" => return Ok(Type::Null),
                "null" => return Ok(Type::Null),
                "bigint" => {
                    return Ok(Type::Integer {
                        bits: 64,
                        signed: true,
                    });
                }
                "object" | "symbol" => return Ok(Type::Any),
                _ => {}
            }
        }
        Ok(Type::Any)
    }

    fn extract_union_type(&self, node: Node) -> Result<Type, ParseError> {
        let mut types = Vec::new();
        self.flatten_union_type(node, &mut types)?;

        // Check if this is T | null or T | undefined (optional pattern)
        if types.len() == 2 {
            let null_idx = types.iter().position(|t| matches!(t, Type::Null));
            if let Some(idx) = null_idx {
                let other_idx = 1 - idx;
                return Ok(Type::Optional(Box::new(types.swap_remove(other_idx))));
            }
        }

        Ok(Type::Union(types))
    }

    fn flatten_union_type(&self, node: Node, out: &mut Vec<Type>) -> Result<(), ParseError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                if child.kind() == "union_type" {
                    self.flatten_union_type(child, out)?;
                } else {
                    out.push(self.extract_type(child)?);
                }
            }
        }
        Ok(())
    }

    fn extract_generic_type(&self, node: Node) -> Result<Type, ParseError> {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(n))
            .unwrap_or("");

        let type_args = node.child_by_field_name("type_arguments");
        let args: Vec<Type> = if let Some(ta) = type_args {
            let mut result = Vec::new();
            let mut cursor = ta.walk();
            for child in ta.children(&mut cursor) {
                if child.is_named() {
                    result.push(self.extract_type(child)?);
                }
            }
            result
        } else {
            Vec::new()
        };

        match name {
            "Array" | "ReadonlyArray" => {
                let inner = args.into_iter().next().unwrap_or(Type::Any);
                Ok(Type::Array(Box::new(inner)))
            }
            "Record" | "Map" => {
                let mut iter = args.into_iter();
                let key = iter.next().unwrap_or(Type::String);
                let value = iter.next().unwrap_or(Type::Any);
                Ok(Type::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                })
            }
            "Set" => {
                let inner = args.into_iter().next().unwrap_or(Type::Any);
                Ok(Type::Array(Box::new(inner)))
            }
            "Promise" => {
                // Unwrap Promise<T> → T
                let inner = args.into_iter().next().unwrap_or(Type::Any);
                Ok(inner)
            }
            "Partial" | "Required" | "Readonly" => {
                // Pass through the inner type
                let inner = args.into_iter().next().unwrap_or(Type::Any);
                Ok(inner)
            }
            "Omit" | "Pick" | "Exclude" | "Extract" => {
                // Utility types - best approximation is Any
                Ok(Type::Any)
            }
            _ => {
                // Unknown generic - treat as Ref
                Ok(Type::Ref(name.to_string()))
            }
        }
    }

    fn extract_literal_type(&self, node: Node) -> Result<Type, ParseError> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "string" | "string_fragment" => {
                    // Extract string literal value
                    let text = self.node_text(child);
                    if text.starts_with('"') || text.starts_with('\'') {
                        let inner = &text[1..text.len() - 1];
                        return Ok(Type::StringLiteral(inner.to_string()));
                    }
                    return Ok(Type::StringLiteral(text.to_string()));
                }
                "number" => {
                    let text = self.node_text(child);
                    if let Ok(n) = text.parse::<i64>() {
                        return Ok(Type::IntLiteral(n));
                    }
                    return Ok(Type::Any);
                }
                "true" => return Ok(Type::BoolLiteral(true)),
                "false" => return Ok(Type::BoolLiteral(false)),
                "null" => return Ok(Type::Null),
                _ => {}
            }
        }
        Ok(Type::Any)
    }

    fn extract_type_alias(&self, node: Node) -> Result<TypeDef, ParseError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ParseError::Unsupported("type alias missing name".into()))?;
        let name_str = self.node_text(name).to_string();

        let value = node
            .child_by_field_name("value")
            .ok_or_else(|| ParseError::Unsupported("type alias missing value".into()))?;

        // Check if this is a union of string literals (→ StringLiteral enum)
        if value.kind() == "union_type" {
            if let Some(enum_def) = self.try_extract_string_literal_enum(value) {
                return Ok(TypeDef {
                    name: name_str,
                    docs: None,
                    kind: TypeDefKind::Enum(enum_def),
                });
            }
        }

        let ty = self.extract_type(value)?;
        Ok(TypeDef {
            name: name_str,
            docs: None,
            kind: TypeDefKind::Alias(ty),
        })
    }

    fn try_extract_string_literal_enum(&self, node: Node) -> Option<EnumDef> {
        let mut variants = Vec::new();
        if !self.collect_string_literal_variants(node, &mut variants) {
            return None;
        }
        if variants.is_empty() {
            return None;
        }
        Some(EnumDef {
            kind: EnumKind::StringLiteral(variants),
        })
    }

    fn collect_string_literal_variants(
        &self,
        node: Node,
        variants: &mut Vec<StringVariant>,
    ) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "union_type" {
                if !self.collect_string_literal_variants(child, variants) {
                    return false;
                }
            } else if child.kind() == "literal_type" {
                if let Some(value) = self.extract_string_literal_value(child) {
                    variants.push(StringVariant { value, docs: None });
                } else {
                    return false;
                }
            }
        }
        true
    }

    fn extract_string_literal_value(&self, node: Node) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                // String node contains quote delimiters and string_fragment
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "string_fragment" {
                        return Some(self.node_text(inner).to_string());
                    }
                }
                // Fallback: strip quotes
                let text = self.node_text(child);
                if text.len() >= 2 {
                    return Some(text[1..text.len() - 1].to_string());
                }
            }
        }
        None
    }

    fn extract_enum(&self, node: Node) -> Result<TypeDef, ParseError> {
        let name = node
            .child_by_field_name("name")
            .ok_or_else(|| ParseError::Unsupported("enum missing name".into()))?;
        let name_str = self.node_text(name).to_string();

        let body = node
            .child_by_field_name("body")
            .ok_or_else(|| ParseError::Unsupported("enum missing body".into()))?;

        let mut string_variants = Vec::new();
        let mut int_variants = Vec::new();
        let mut auto_index: i64 = 0;
        let mut is_string = false;
        let mut cursor = body.walk();

        for child in body.children(&mut cursor) {
            match child.kind() {
                "enum_assignment" => {
                    let member_name = child
                        .child_by_field_name("name")
                        .map(|n| self.node_text(n).to_string())
                        .unwrap_or_default();
                    let value = child.child_by_field_name("value");

                    if let Some(val_node) = value {
                        let val_text = self.node_text(val_node);
                        if val_node.kind() == "string"
                            || (val_text.starts_with('"') || val_text.starts_with('\''))
                        {
                            is_string = true;
                            // Extract string value
                            let inner = self.extract_enum_string_value(val_node);
                            string_variants.push(StringVariant {
                                value: inner,
                                docs: None,
                            });
                        } else if let Ok(n) = val_text.parse::<i64>() {
                            int_variants.push(IntVariant {
                                value: n,
                                name: Some(member_name),
                                docs: None,
                            });
                            auto_index = n + 1;
                        }
                    } else {
                        // No value, auto-increment
                        int_variants.push(IntVariant {
                            value: auto_index,
                            name: Some(member_name),
                            docs: None,
                        });
                        auto_index += 1;
                    }
                }
                "property_identifier" => {
                    // Bare enum member (no assignment)
                    let member_name = self.node_text(child).to_string();
                    int_variants.push(IntVariant {
                        value: auto_index,
                        name: Some(member_name),
                        docs: None,
                    });
                    auto_index += 1;
                }
                _ => {}
            }
        }

        let kind = if is_string {
            EnumKind::StringLiteral(string_variants)
        } else {
            EnumKind::IntLiteral(int_variants)
        };

        Ok(TypeDef {
            name: name_str,
            docs: None,
            kind: TypeDefKind::Enum(EnumDef { kind }),
        })
    }

    fn extract_enum_string_value(&self, node: Node) -> String {
        // Node is a "string" which contains string_fragment
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string_fragment" {
                return self.node_text(child).to_string();
            }
        }
        // Fallback: strip quotes from text
        let text = self.node_text(node);
        if text.len() >= 2 && (text.starts_with('"') || text.starts_with('\'')) {
            text[1..text.len() - 1].to_string()
        } else {
            text.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interface() {
        let schema = parse_typescript_types(
            r#"
            interface User {
                id: string;
                name: string;
                age?: number;
            }
            "#,
        )
        .unwrap();
        assert_eq!(schema.definitions.len(), 1);
        let def = &schema.definitions[0];
        assert_eq!(def.name, "User");
        match &def.kind {
            TypeDefKind::Struct(s) => {
                assert_eq!(s.fields.len(), 3);
                assert_eq!(s.fields[0].name, "id");
                assert!(s.fields[0].required);
                assert!(matches!(s.fields[0].ty, Type::String));
                assert_eq!(s.fields[2].name, "age");
                assert!(!s.fields[2].required);
                assert!(matches!(s.fields[2].ty, Type::Float { bits: 64 }));
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_type_alias_string_enum() {
        let schema =
            parse_typescript_types(r#"type Status = "active" | "inactive" | "pending";"#).unwrap();
        assert_eq!(schema.definitions.len(), 1);
        match &schema.definitions[0].kind {
            TypeDefKind::Enum(EnumDef {
                kind: EnumKind::StringLiteral(variants),
            }) => {
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].value, "active");
                assert_eq!(variants[1].value, "inactive");
                assert_eq!(variants[2].value, "pending");
            }
            _ => panic!("expected StringLiteral enum"),
        }
    }

    #[test]
    fn test_type_alias_simple() {
        let schema = parse_typescript_types("type UserId = string;").unwrap();
        assert_eq!(schema.definitions.len(), 1);
        match &schema.definitions[0].kind {
            TypeDefKind::Alias(Type::String) => {}
            _ => panic!("expected Alias(String)"),
        }
    }

    #[test]
    fn test_enum_string() {
        let schema = parse_typescript_types(
            r#"
            enum Color {
                Red = "red",
                Green = "green",
                Blue = "blue",
            }
            "#,
        )
        .unwrap();
        match &schema.definitions[0].kind {
            TypeDefKind::Enum(EnumDef {
                kind: EnumKind::StringLiteral(variants),
            }) => {
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].value, "red");
            }
            _ => panic!("expected StringLiteral enum"),
        }
    }

    #[test]
    fn test_enum_numeric() {
        let schema = parse_typescript_types(
            r#"
            enum Direction {
                Up,
                Down,
                Left,
                Right,
            }
            "#,
        )
        .unwrap();
        match &schema.definitions[0].kind {
            TypeDefKind::Enum(EnumDef {
                kind: EnumKind::IntLiteral(variants),
            }) => {
                assert_eq!(variants.len(), 4);
                assert_eq!(variants[0].value, 0);
                assert_eq!(variants[0].name.as_deref(), Some("Up"));
                assert_eq!(variants[3].value, 3);
            }
            _ => panic!("expected IntLiteral enum"),
        }
    }

    #[test]
    fn test_array_types() {
        let schema = parse_typescript_types(
            r#"
            interface Lists {
                tags: string[];
                items: Array<number>;
            }
            "#,
        )
        .unwrap();
        match &schema.definitions[0].kind {
            TypeDefKind::Struct(s) => {
                assert!(
                    matches!(&s.fields[0].ty, Type::Array(inner) if matches!(inner.as_ref(), Type::String))
                );
                assert!(
                    matches!(&s.fields[1].ty, Type::Array(inner) if matches!(inner.as_ref(), Type::Float { bits: 64 }))
                );
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_nullable_union() {
        let schema = parse_typescript_types(
            r#"
            interface Opt {
                value: string | null;
            }
            "#,
        )
        .unwrap();
        match &schema.definitions[0].kind {
            TypeDefKind::Struct(s) => {
                assert!(
                    matches!(&s.fields[0].ty, Type::Optional(inner) if matches!(inner.as_ref(), Type::String))
                );
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_record_type() {
        let schema = parse_typescript_types(
            r#"
            interface Config {
                metadata: Record<string, unknown>;
            }
            "#,
        )
        .unwrap();
        match &schema.definitions[0].kind {
            TypeDefKind::Struct(s) => {
                assert!(matches!(&s.fields[0].ty, Type::Map { .. }));
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_export_interface() {
        let schema = parse_typescript_types(
            r#"
            export interface Product {
                id: string;
                price: number;
            }
            "#,
        )
        .unwrap();
        assert_eq!(schema.definitions.len(), 1);
        assert_eq!(schema.definitions[0].name, "Product");
    }

    #[test]
    fn test_doc_comment() {
        let schema = parse_typescript_types(
            r#"
            /** A user in the system. */
            interface User {
                /** The user's unique identifier. */
                id: string;
            }
            "#,
        )
        .unwrap();
        assert_eq!(
            schema.definitions[0].docs.as_deref(),
            Some("A user in the system.")
        );
        match &schema.definitions[0].kind {
            TypeDefKind::Struct(s) => {
                assert_eq!(
                    s.fields[0].docs.as_deref(),
                    Some("The user's unique identifier.")
                );
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_type_ref() {
        let schema = parse_typescript_types(
            r#"
            type UserId = string;
            interface User {
                id: UserId;
            }
            "#,
        )
        .unwrap();
        assert_eq!(schema.definitions.len(), 2);
        match &schema.definitions[1].kind {
            TypeDefKind::Struct(s) => {
                assert!(matches!(&s.fields[0].ty, Type::Ref(name) if name == "UserId"));
            }
            _ => panic!("expected Struct"),
        }
    }
}
