//! JSON Schema to IR parser.

use crate::ir::{
    EnumDef, EnumKind, Field, Schema, StringVariant, StructDef, TaggedUnion, TaggedVariant, Type,
    TypeDef, TypeDefKind,
};
use serde_json::Value;

/// Parse a JSON Schema document into an IR Schema.
pub fn parse_json_schema(input: &Value) -> Result<Schema, ParseError> {
    let mut schema = Schema::new();
    let mut parser = Parser::new();

    // Handle $defs / definitions
    if let Some(defs) = input.get("$defs").or_else(|| input.get("definitions")) {
        if let Some(obj) = defs.as_object() {
            for (name, def) in obj {
                if let Some(type_def) = parser.parse_definition(name, def)? {
                    schema.add(type_def);
                }
            }
        }
    }

    // Handle root schema if it defines a type
    if input.get("type").is_some() || input.get("properties").is_some() {
        let root_name = input
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Root");
        if let Some(type_def) = parser.parse_definition(root_name, input)? {
            schema.add(type_def);
        }
    }

    Ok(schema)
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unsupported schema construct: {0}")]
    Unsupported(String),
}

struct Parser {
    // Could track state like seen refs, etc.
}

impl Parser {
    fn new() -> Self {
        Self {}
    }

    fn parse_definition(
        &mut self,
        name: &str,
        schema: &Value,
    ) -> Result<Option<TypeDef>, ParseError> {
        let docs = schema
            .get("description")
            .and_then(|d| d.as_str())
            .map(String::from);

        // Check for enum
        if let Some(enum_values) = schema.get("enum") {
            return Ok(Some(self.parse_enum(name, enum_values, docs)?));
        }

        // Check for oneOf with discriminator (tagged union)
        if let Some(one_of) = schema.get("oneOf") {
            if let Some(disc) = schema.get("discriminator") {
                return Ok(Some(self.parse_tagged_union(name, one_of, disc, docs)?));
            }
        }

        // Check for object type
        let type_val = schema.get("type").and_then(|t| t.as_str());
        if type_val == Some("object") || schema.get("properties").is_some() {
            return Ok(Some(self.parse_struct(name, schema, docs)?));
        }

        // Simple type alias
        if let Some(ty) = self.parse_type(schema)? {
            return Ok(Some(TypeDef {
                name: name.to_string(),
                docs,
                kind: TypeDefKind::Alias(ty),
            }));
        }

        Ok(None)
    }

    fn parse_struct(
        &mut self,
        name: &str,
        schema: &Value,
        docs: Option<String>,
    ) -> Result<TypeDef, ParseError> {
        let mut fields = Vec::new();

        let required: Vec<&str> = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (prop_name, prop_schema) in props {
                let ty = self.parse_type(prop_schema)?.unwrap_or(Type::Any);
                let field_docs = prop_schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from);

                let mut field = if required.contains(&prop_name.as_str()) {
                    Field::required(prop_name.clone(), ty)
                } else {
                    Field::optional(prop_name.clone(), ty)
                };

                if let Some(d) = field_docs {
                    field = field.with_docs(d);
                }

                fields.push(field);
            }
        }

        Ok(TypeDef {
            name: name.to_string(),
            docs,
            kind: TypeDefKind::Struct(StructDef { fields }),
        })
    }

    fn parse_enum(
        &mut self,
        name: &str,
        values: &Value,
        docs: Option<String>,
    ) -> Result<TypeDef, ParseError> {
        let arr = values
            .as_array()
            .ok_or_else(|| ParseError::Unsupported("enum must be an array".into()))?;

        // Check if all values are strings
        let all_strings = arr.iter().all(|v| v.is_string());
        let all_ints = arr.iter().all(|v| v.is_i64());

        let kind = if all_strings {
            EnumKind::StringLiteral(
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| StringVariant {
                        value: s.to_string(),
                        docs: None,
                    })
                    .collect(),
            )
        } else if all_ints {
            EnumKind::IntLiteral(
                arr.iter()
                    .filter_map(|v| v.as_i64())
                    .map(|i| crate::ir::IntVariant {
                        value: i,
                        name: None,
                        docs: None,
                    })
                    .collect(),
            )
        } else {
            return Err(ParseError::Unsupported(
                "mixed-type enums not supported".into(),
            ));
        };

        Ok(TypeDef {
            name: name.to_string(),
            docs,
            kind: TypeDefKind::Enum(EnumDef { kind }),
        })
    }

    fn parse_tagged_union(
        &mut self,
        name: &str,
        one_of: &Value,
        discriminator: &Value,
        docs: Option<String>,
    ) -> Result<TypeDef, ParseError> {
        let disc_prop = discriminator
            .get("propertyName")
            .and_then(|p| p.as_str())
            .ok_or_else(|| {
                ParseError::Unsupported("discriminator must have propertyName".into())
            })?;

        let variants_arr = one_of
            .as_array()
            .ok_or_else(|| ParseError::Unsupported("oneOf must be an array".into()))?;

        let mut variants = Vec::new();

        for variant_schema in variants_arr {
            // Get the discriminator value from const or enum
            let tag = variant_schema
                .pointer(&format!("/properties/{}/const", disc_prop))
                .and_then(|c| c.as_str())
                .or_else(|| {
                    variant_schema
                        .pointer(&format!("/properties/{}/enum/0", disc_prop))
                        .and_then(|e| e.as_str())
                })
                .ok_or_else(|| {
                    ParseError::Unsupported("variant must have discriminator const/enum".into())
                })?;

            let variant_docs = variant_schema
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from);

            // Parse fields (excluding discriminator)
            let mut fields = Vec::new();
            let required: Vec<&str> = variant_schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            if let Some(props) = variant_schema.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_schema) in props {
                    if prop_name == disc_prop {
                        continue; // Skip discriminator field
                    }
                    let ty = self.parse_type(prop_schema)?.unwrap_or(Type::Any);
                    let field = if required.contains(&prop_name.as_str()) {
                        Field::required(prop_name.clone(), ty)
                    } else {
                        Field::optional(prop_name.clone(), ty)
                    };
                    fields.push(field);
                }
            }

            variants.push(TaggedVariant {
                tag: tag.to_string(),
                fields,
                docs: variant_docs,
            });
        }

        Ok(TypeDef {
            name: name.to_string(),
            docs,
            kind: TypeDefKind::Enum(EnumDef {
                kind: EnumKind::Tagged(TaggedUnion {
                    discriminator: disc_prop.to_string(),
                    variants,
                }),
            }),
        })
    }

    fn parse_type(&mut self, schema: &Value) -> Result<Option<Type>, ParseError> {
        // Handle $ref
        if let Some(ref_path) = schema.get("$ref").and_then(|r| r.as_str()) {
            let type_name = ref_path.rsplit('/').next().unwrap_or(ref_path);
            return Ok(Some(Type::Ref(type_name.to_string())));
        }

        // Handle const
        if let Some(const_val) = schema.get("const") {
            return Ok(Some(self.parse_const(const_val)?));
        }

        // Handle type array (union with null)
        if let Some(arr) = schema.get("type").and_then(|t| t.as_array()) {
            let types: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            let has_null = types.contains(&"null");
            let non_null: Vec<_> = types.iter().filter(|t| **t != "null").collect();

            if non_null.len() == 1 {
                let base = self.parse_type_string(non_null[0], schema)?;
                return Ok(Some(if has_null {
                    Type::Optional(Box::new(base))
                } else {
                    base
                }));
            }
        }

        // Handle type string
        if let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) {
            return Ok(Some(self.parse_type_string(type_str, schema)?));
        }

        // Handle anyOf / oneOf without discriminator
        if let Some(any_of) = schema.get("anyOf").or_else(|| schema.get("oneOf")) {
            if let Some(arr) = any_of.as_array() {
                let types: Vec<Type> = arr
                    .iter()
                    .filter_map(|s| self.parse_type(s).ok().flatten())
                    .collect();
                if types.len() == 1 {
                    return Ok(Some(types.into_iter().next().unwrap()));
                }
                if !types.is_empty() {
                    return Ok(Some(Type::Union(types)));
                }
            }
        }

        // Handle allOf (intersection - flatten into first type for now)
        if let Some(all_of) = schema.get("allOf") {
            if let Some(arr) = all_of.as_array() {
                if let Some(first) = arr.first() {
                    return self.parse_type(first);
                }
            }
        }

        Ok(None)
    }

    fn parse_type_string(&mut self, type_str: &str, schema: &Value) -> Result<Type, ParseError> {
        Ok(match type_str {
            "string" => {
                // Check for format
                match schema.get("format").and_then(|f| f.as_str()) {
                    Some("date-time") | Some("date") | Some("time") => Type::String, // Could add Date type later
                    _ => Type::String,
                }
            }
            "integer" => Type::Integer {
                bits: 64,
                signed: true,
            },
            "number" => Type::Float { bits: 64 },
            "boolean" => Type::Boolean,
            "null" => Type::Null,
            "array" => {
                let items = schema
                    .get("items")
                    .and_then(|i| self.parse_type(i).ok().flatten())
                    .unwrap_or(Type::Any);
                Type::Array(Box::new(items))
            }
            "object" => {
                // Check for additionalProperties (map type)
                if let Some(add_props) = schema.get("additionalProperties") {
                    if add_props.is_boolean() && add_props.as_bool() == Some(false) {
                        // Closed object with no additional props - just return Any for inline objects
                        return Ok(Type::Any);
                    }
                    if add_props.is_object() {
                        let value_type = self.parse_type(add_props)?.unwrap_or(Type::Any);
                        return Ok(Type::Map {
                            key: Box::new(Type::String),
                            value: Box::new(value_type),
                        });
                    }
                }
                Type::Any
            }
            _ => Type::Any,
        })
    }

    fn parse_const(&mut self, value: &Value) -> Result<Type, ParseError> {
        Ok(match value {
            Value::String(s) => Type::StringLiteral(s.clone()),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Type::IntLiteral(i)
                } else {
                    Type::Any
                }
            }
            Value::Bool(b) => Type::BoolLiteral(*b),
            Value::Null => Type::Null,
            _ => Type::Any,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple_object() {
        let input = json!({
            "type": "object",
            "title": "User",
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["id", "name"]
        });

        let schema = parse_json_schema(&input).unwrap();
        assert_eq!(schema.definitions.len(), 1);
        assert_eq!(schema.definitions[0].name, "User");

        if let TypeDefKind::Struct(s) = &schema.definitions[0].kind {
            assert_eq!(s.fields.len(), 3);
            assert!(s.fields.iter().find(|f| f.name == "id").unwrap().required);
            assert!(!s.fields.iter().find(|f| f.name == "age").unwrap().required);
        } else {
            panic!("expected struct");
        }
    }

    #[test]
    fn parse_string_enum() {
        let input = json!({
            "$defs": {
                "Status": {
                    "enum": ["pending", "active", "done"]
                }
            }
        });

        let schema = parse_json_schema(&input).unwrap();
        assert_eq!(schema.definitions.len(), 1);

        if let TypeDefKind::Enum(e) = &schema.definitions[0].kind {
            if let EnumKind::StringLiteral(variants) = &e.kind {
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].value, "pending");
            } else {
                panic!("expected string literal enum");
            }
        } else {
            panic!("expected enum");
        }
    }

    #[test]
    fn parse_ref() {
        let input = json!({
            "$defs": {
                "Status": {
                    "enum": ["pending", "active"]
                },
                "User": {
                    "type": "object",
                    "properties": {
                        "status": { "$ref": "#/$defs/Status" }
                    }
                }
            }
        });

        let schema = parse_json_schema(&input).unwrap();
        assert_eq!(schema.definitions.len(), 2);

        let user = schema
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .unwrap();
        if let TypeDefKind::Struct(s) = &user.kind {
            let status_field = s.fields.iter().find(|f| f.name == "status").unwrap();
            assert!(matches!(&status_field.ty, Type::Ref(name) if name == "Status"));
        }
    }
}
