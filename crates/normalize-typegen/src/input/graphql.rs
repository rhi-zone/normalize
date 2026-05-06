//! GraphQL SDL (Schema Definition Language) to IR parser.
//!
//! Extracts type definitions from GraphQL schema files
//! (`type`, `enum`, and `input` declarations) into the typegen IR.
//!
//! Uses the tree-sitter `arborium-graphql` grammar for parsing.
//! Requires the `input-graphql` feature flag.

use super::ParseError;
use crate::ir::{EnumDef, EnumKind, Field, Schema, StringVariant, Type, TypeDef, TypeDefKind};
use tree_sitter::{Node, Parser, Tree};

/// Parse a GraphQL SDL string and extract type definitions into IR.
pub fn parse_graphql_schema(source: &str) -> Result<Schema, ParseError> {
    let mut parser = Parser::new();
    parser
        .set_language(&arborium_graphql::language().into())
        .map_err(|e| ParseError::Unsupported(format!("tree-sitter init: {}", e)))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::Unsupported("failed to parse GraphQL SDL".into()))?;

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
        self.collect_type_definitions(root, &mut schema)?;
        Ok(schema)
    }

    /// Recursively walk the tree and collect type definitions.
    ///
    /// The GraphQL grammar wraps top-level definitions in multiple container
    /// nodes: `source_file → document → definition → type_system_definition →
    /// type_definition → object_type_definition`. We recurse through the
    /// wrappers until we hit recognizable definition nodes.
    fn collect_type_definitions(&self, node: Node, schema: &mut Schema) -> Result<(), ParseError> {
        match node.kind() {
            // `type Foo { ... }` — object type definitions
            "object_type_definition" => {
                schema.add(self.extract_object_type(node)?);
            }
            // `input Foo { ... }` — input object type definitions (treated as structs)
            "input_object_type_definition" => {
                schema.add(self.extract_input_type(node)?);
            }
            // `enum Foo { ... }` — enum definitions
            "enum_type_definition" => {
                schema.add(self.extract_enum(node)?);
            }
            // `interface Foo { ... }` — interface definitions (treated as structs)
            "interface_type_definition" => {
                schema.add(self.extract_interface_type(node)?);
            }
            // Wrapper nodes — recurse into children
            _ => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.collect_type_definitions(child, schema)?;
                }
            }
        }
        Ok(())
    }

    /// Extract the description string (block string or regular string) from a node.
    fn extract_description(&self, node: Node) -> Option<String> {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "description"
            {
                // description node wraps a string_value or block_string_value
                let text = self.node_text(child);
                return Some(clean_graphql_description(text));
            }
        }
        None
    }

    /// Extract `object_type_definition`: `type Name { fields }`
    fn extract_object_type(&self, node: Node) -> Result<TypeDef, ParseError> {
        let name = self.get_name(node)?;
        let docs = self.extract_description(node);
        let fields = self.extract_fields_definition(node)?;
        let mut def = TypeDef::structure(name, fields);
        def.docs = docs;
        Ok(def)
    }

    /// Extract `input_object_type_definition`: `input Name { fields }`
    fn extract_input_type(&self, node: Node) -> Result<TypeDef, ParseError> {
        let name = self.get_name(node)?;
        let docs = self.extract_description(node);
        let fields = self.extract_input_fields_definition(node)?;
        let mut def = TypeDef::structure(name, fields);
        def.docs = docs;
        Ok(def)
    }

    /// Extract `interface_type_definition`: `interface Name { fields }`
    fn extract_interface_type(&self, node: Node) -> Result<TypeDef, ParseError> {
        let name = self.get_name(node)?;
        let docs = self.extract_description(node);
        let fields = self.extract_fields_definition(node)?;
        let mut def = TypeDef::structure(name, fields);
        def.docs = docs;
        Ok(def)
    }

    /// Extract `enum_type_definition`: `enum Name { VALUES }`
    fn extract_enum(&self, node: Node) -> Result<TypeDef, ParseError> {
        let name = self.get_name(node)?;
        let docs = self.extract_description(node);
        let mut variants = Vec::new();

        // Find enum_values_definition child
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "enum_values_definition"
            {
                let mut cursor = child.walk();
                for value_node in child.children(&mut cursor) {
                    if value_node.kind() == "enum_value_definition" {
                        let variant_docs = self.extract_description(value_node);
                        // enum_value_definition has an enum_value child which has a name child
                        let variant_name = self.get_enum_value_name(value_node)?;
                        variants.push(StringVariant {
                            value: variant_name,
                            docs: variant_docs,
                        });
                    }
                }
            }
        }

        let def = TypeDef {
            name: name.to_string(),
            docs,
            kind: TypeDefKind::Enum(EnumDef {
                kind: EnumKind::StringLiteral(variants),
            }),
        };
        Ok(def)
    }

    /// Get the name of an enum value from an `enum_value_definition` node.
    fn get_enum_value_name(&self, node: Node) -> Result<String, ParseError> {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "enum_value"
            {
                // enum_value has a name child
                for j in 0..child.child_count() {
                    if let Some(name_node) = child.child(j as u32)
                        && name_node.kind() == "name"
                    {
                        return Ok(self.node_text(name_node).to_string());
                    }
                }
            }
        }
        Err(ParseError::Unsupported(
            "enum_value_definition missing enum_value name".into(),
        ))
    }

    /// Extract fields from `fields_definition` (for object/interface types).
    fn extract_fields_definition(&self, node: Node) -> Result<Vec<Field>, ParseError> {
        let mut fields = Vec::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "fields_definition"
            {
                let mut cursor = child.walk();
                for field_node in child.children(&mut cursor) {
                    if field_node.kind() == "field_definition" {
                        fields.push(self.extract_field_definition(field_node)?);
                    }
                }
            }
        }
        Ok(fields)
    }

    /// Extract fields from `input_fields_definition` (for input types).
    fn extract_input_fields_definition(&self, node: Node) -> Result<Vec<Field>, ParseError> {
        let mut fields = Vec::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "input_fields_definition"
            {
                let mut cursor = child.walk();
                for field_node in child.children(&mut cursor) {
                    if field_node.kind() == "input_value_definition" {
                        fields.push(self.extract_input_value_definition(field_node)?);
                    }
                }
            }
        }
        Ok(fields)
    }

    /// Extract a `field_definition` node: `name: Type`
    fn extract_field_definition(&self, node: Node) -> Result<Field, ParseError> {
        let name = self.get_name(node)?;
        let docs = self.extract_description(node);
        let mut field = self.make_field(name, node)?;
        field.docs = docs;
        Ok(field)
    }

    /// Extract an `input_value_definition` node: `name: Type`
    fn extract_input_value_definition(&self, node: Node) -> Result<Field, ParseError> {
        let name = self.get_name(node)?;
        let docs = self.extract_description(node);
        let mut field = self.make_field(name, node)?;
        field.docs = docs;
        Ok(field)
    }

    /// Build a `Field` from a field or input_value_definition node.
    ///
    /// In GraphQL SDL, nullable types (the default) are represented as
    /// `Type::Optional(T)` in the IR with `required=false`.
    /// Non-null types (`!`) are represented as `T` with `required=true`.
    fn make_field(&self, name: String, node: Node) -> Result<Field, ParseError> {
        let (ty, required) = self.extract_type_from_field_def(node)?;
        if required {
            Ok(Field::required(name, ty))
        } else {
            // Nullable GraphQL field: wrap the type in Optional
            Ok(Field::optional(name, Type::Optional(Box::new(ty))))
        }
    }

    /// Find the type node in a field/input definition and extract the IR Type plus nullability.
    ///
    /// In GraphQL SDL:
    /// - `String`   → nullable → `Optional(String)`
    /// - `String!`  → non-null → `String`
    /// - `[String]` → nullable list → `Optional(Array(Optional(String)))`
    /// - `[String!]!` → non-null list of non-null → `Array(String)`
    ///
    /// The grammar wraps the actual type in a `type` node:
    ///   `field_definition → type → non_null_type / named_type / list_type`
    fn extract_type_from_field_def(&self, node: Node) -> Result<(Type, bool), ParseError> {
        for i in 0..node.child_count() {
            let child = match node.child(i as u32) {
                Some(c) => c,
                None => continue,
            };
            match child.kind() {
                // Grammar wraps the concrete type in a `type` node
                "type" => {
                    for j in 0..child.child_count() {
                        if let Some(inner) = child.child(j as u32) {
                            match inner.kind() {
                                "named_type" | "list_type" | "non_null_type" => {
                                    return self.extract_graphql_type(inner);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                // Direct (no wrapper) — kept for resilience
                "named_type" | "list_type" | "non_null_type" => {
                    return self.extract_graphql_type(child);
                }
                _ => {}
            }
        }
        Err(ParseError::Unsupported(format!(
            "field definition missing type annotation: {}",
            self.node_text(node)
        )))
    }

    /// Recursively extract a GraphQL type node into `(Type, is_required)`.
    ///
    /// GraphQL's nullability model: everything is nullable by default.
    /// `non_null_type` wraps either `named_type` or `list_type` to make it required.
    fn extract_graphql_type(&self, node: Node) -> Result<(Type, bool), ParseError> {
        match node.kind() {
            "non_null_type" => {
                // non_null_type wraps a named_type or list_type — the inner type is required
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i as u32) {
                        match child.kind() {
                            "named_type" | "list_type" => {
                                let (inner_ty, _) = self.extract_graphql_type(child)?;
                                // non_null means not wrapped in Optional
                                return Ok((inner_ty, true));
                            }
                            _ => {}
                        }
                    }
                }
                Err(ParseError::Unsupported(
                    "non_null_type missing inner type".into(),
                ))
            }
            "list_type" => {
                // list_type wraps its element type in a `type` node:
                //   list_type → `[` → type → (named_type | list_type | non_null_type) → `]`
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i as u32) {
                        // Unwrap the `type` container node
                        let type_child = if child.kind() == "type" {
                            // Find the concrete type inside the `type` wrapper
                            let mut inner = None;
                            for j in 0..child.child_count() {
                                if let Some(gc) = child.child(j as u32) {
                                    match gc.kind() {
                                        "named_type" | "list_type" | "non_null_type" => {
                                            inner = Some(gc);
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            inner
                        } else {
                            match child.kind() {
                                "named_type" | "list_type" | "non_null_type" => Some(child),
                                _ => None,
                            }
                        };

                        if let Some(tc) = type_child {
                            let (inner_ty, inner_required) = self.extract_graphql_type(tc)?;
                            // If inner not required, wrap it in Optional
                            let element_ty = if inner_required {
                                inner_ty
                            } else {
                                Type::Optional(Box::new(inner_ty))
                            };
                            // list_type itself is nullable (no wrapping non_null_type)
                            return Ok((Type::Array(Box::new(element_ty)), false));
                        }
                    }
                }
                Err(ParseError::Unsupported(
                    "list_type missing inner type".into(),
                ))
            }
            "named_type" => {
                // named_type has a name child
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i as u32)
                        && child.kind() == "name"
                    {
                        let type_name = self.node_text(child);
                        let ty = graphql_builtin_type(type_name);
                        // named_type is nullable by default
                        return Ok((ty, false));
                    }
                }
                Err(ParseError::Unsupported(
                    "named_type missing name child".into(),
                ))
            }
            other => Err(ParseError::Unsupported(format!(
                "unexpected type node kind: {}",
                other
            ))),
        }
    }

    /// Get the `name` field value from a node.
    fn get_name(&self, node: Node) -> Result<String, ParseError> {
        // First try field-named child "name"
        if let Some(name_node) = node.child_by_field_name("name") {
            return Ok(self.node_text(name_node).to_string());
        }
        // GraphQL grammar doesn't always use field names — scan for `name` child
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32)
                && child.kind() == "name"
            {
                return Ok(self.node_text(child).to_string());
            }
        }
        Err(ParseError::Unsupported(format!(
            "node missing name: {}",
            node.kind()
        )))
    }
}

/// Map GraphQL built-in scalar types to IR primitives.
/// Unknown types map to `Type::Ref`.
fn graphql_builtin_type(name: &str) -> Type {
    match name {
        "String" => Type::String,
        "Int" => Type::Integer {
            bits: 32,
            signed: true,
        },
        "Float" => Type::Float { bits: 64 },
        "Boolean" => Type::Boolean,
        "ID" => Type::String, // IDs are string-like in practice
        other => Type::Ref(other.to_string()),
    }
}

/// Clean a GraphQL description value (block string `"""..."""` or regular `"..."`).
fn clean_graphql_description(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(inner) = raw
        .strip_prefix("\"\"\"")
        .and_then(|s| s.strip_suffix("\"\"\""))
    {
        // Block string: strip leading/trailing whitespace from each line, drop empty lines
        inner
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else if let Some(inner) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        inner.to_string()
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EnumKind, TypeDefKind};

    const SAMPLE_SDL: &str = r#"
"""A user in the system"""
type User {
  id: ID!
  name: String!
  email: String
  tags: [String!]!
  address: Address
  status: Status!
}

type Address {
  street: String!
  city: String!
}

enum Status {
  """Unknown status"""
  UNKNOWN
  ACTIVE
  INACTIVE
}

input CreateUserInput {
  name: String!
  email: String
}
"#;

    #[test]
    fn parse_graphql_object_type() {
        let schema = parse_graphql_schema(SAMPLE_SDL).expect("parse failed");
        let user = schema
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .expect("User not found");
        assert_eq!(user.docs.as_deref(), Some("A user in the system"));
        let TypeDefKind::Struct(s) = &user.kind else {
            panic!("expected struct");
        };
        assert_eq!(s.fields.len(), 6);

        // id: ID! → required String
        let id_field = s.fields.iter().find(|f| f.name == "id").unwrap();
        assert!(id_field.required);
        assert!(matches!(id_field.ty, Type::String));

        // email: String → optional String
        let email_field = s.fields.iter().find(|f| f.name == "email").unwrap();
        assert!(!email_field.required);
        assert!(matches!(email_field.ty, Type::Optional(_)));

        // tags: [String!]! → required Array(String)
        let tags_field = s.fields.iter().find(|f| f.name == "tags").unwrap();
        assert!(tags_field.required);
        assert!(matches!(tags_field.ty, Type::Array(_)));

        // address: Address → optional Ref
        let addr_field = s.fields.iter().find(|f| f.name == "address").unwrap();
        assert!(!addr_field.required);
        assert!(matches!(addr_field.ty, Type::Optional(_)));
    }

    #[test]
    fn parse_graphql_enum() {
        let schema = parse_graphql_schema(SAMPLE_SDL).expect("parse failed");
        let status = schema
            .definitions
            .iter()
            .find(|d| d.name == "Status")
            .expect("Status not found");
        let TypeDefKind::Enum(e) = &status.kind else {
            panic!("expected enum");
        };
        let EnumKind::StringLiteral(variants) = &e.kind else {
            panic!("expected string literal enum");
        };
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].value, "UNKNOWN");
        assert_eq!(variants[0].docs.as_deref(), Some("Unknown status"));
        assert_eq!(variants[1].value, "ACTIVE");
        assert_eq!(variants[2].value, "INACTIVE");
    }

    #[test]
    fn parse_graphql_input_type() {
        let schema = parse_graphql_schema(SAMPLE_SDL).expect("parse failed");
        let input = schema
            .definitions
            .iter()
            .find(|d| d.name == "CreateUserInput")
            .expect("CreateUserInput not found");
        let TypeDefKind::Struct(s) = &input.kind else {
            panic!("expected struct");
        };
        assert_eq!(s.fields.len(), 2);
        let name_field = s.fields.iter().find(|f| f.name == "name").unwrap();
        assert!(name_field.required);
        let email_field = s.fields.iter().find(|f| f.name == "email").unwrap();
        assert!(!email_field.required);
    }

    #[test]
    fn parse_graphql_all_type_counts() {
        let schema = parse_graphql_schema(SAMPLE_SDL).expect("parse failed");
        // User, Address, Status, CreateUserInput
        assert_eq!(schema.definitions.len(), 4);
    }
}
