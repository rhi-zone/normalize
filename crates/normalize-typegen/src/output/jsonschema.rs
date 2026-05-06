//! JSON Schema output backend.
//!
//! Emits a JSON Schema (draft 2020-12) document from the IR.  Each top-level
//! type is emitted as a `$defs` entry; the root schema is a stub that
//! references all definitions via `anyOf`.
//!
//! Mapping:
//! - Structs → `{ "type": "object", "properties": { … }, "required": [ … ] }`
//! - String-literal enums → `{ "enum": [ … ] }`
//! - Int-literal enums → `{ "type": "integer", "enum": [ … ] }`
//! - Tagged unions → `{ "oneOf": [ … ] }` with a `const` discriminator
//! - `Type::Ref` → `{ "$ref": "#/$defs/Name" }`
//! - `Type::Optional` → the inner type (nullability is expressed on the field)
//! - `nullable: true` on a field wraps its schema with `anyOf: [schema, {"type":"null"}]`
//! - Constraints → `minLength`, `maxLength`, `minimum`, `maximum`, `pattern`, `format`
//! - Default values → `"default"` keyword

use serde_json::{Value, json};

use crate::ir::{
    DefaultValue, EnumKind, Field, FieldConstraints, Schema, Type, TypeDef, TypeDefKind,
};
use crate::traits::{Backend, BackendCategory};

/// Generate a JSON Schema document from an IR schema.
///
/// Returns a pretty-printed JSON string.
pub fn generate_json_schema(schema: &Schema) -> String {
    let root = build_json_schema(schema);
    // normalize-syntax-allow: rust/unwrap-in-impl - serde_json::Value is always serializable
    serde_json::to_string_pretty(&root).unwrap()
}

fn build_json_schema(schema: &Schema) -> Value {
    let mut defs = serde_json::Map::new();

    for def in &schema.definitions {
        defs.insert(def.name.clone(), typedef_to_schema(def));
    }

    let mut root = serde_json::Map::new();
    root.insert(
        "$schema".to_string(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );

    if !defs.is_empty() {
        root.insert("$defs".to_string(), Value::Object(defs));

        // Root is an anyOf over all top-level definitions so the document is
        // self-contained and usable as-is.
        let any_of: Vec<Value> = schema
            .definitions
            .iter()
            .map(|d| json!({ "$ref": format!("#/$defs/{}", d.name) }))
            .collect();
        root.insert("anyOf".to_string(), Value::Array(any_of));
    }

    Value::Object(root)
}

fn typedef_to_schema(def: &TypeDef) -> Value {
    let mut schema = match &def.kind {
        TypeDefKind::Struct(s) => {
            let mut props = serde_json::Map::new();
            let mut required_names: Vec<Value> = Vec::new();

            for field in &s.fields {
                let field_schema = field_to_schema(field);
                props.insert(field.name.clone(), field_schema);
                if field.required {
                    required_names.push(json!(field.name));
                }
            }

            let mut obj = serde_json::Map::new();
            obj.insert("type".to_string(), json!("object"));
            obj.insert("properties".to_string(), Value::Object(props));
            if !required_names.is_empty() {
                obj.insert("required".to_string(), Value::Array(required_names));
            }
            obj.insert("additionalProperties".to_string(), json!(false));
            Value::Object(obj)
        }

        TypeDefKind::Enum(e) => match &e.kind {
            EnumKind::StringLiteral(variants) => {
                let values: Vec<Value> = variants.iter().map(|v| json!(v.value)).collect();
                json!({ "enum": values })
            }
            EnumKind::IntLiteral(variants) => {
                let values: Vec<Value> = variants.iter().map(|v| json!(v.value)).collect();
                json!({ "type": "integer", "enum": values })
            }
            EnumKind::Tagged(tagged) => {
                let one_of: Vec<Value> = tagged
                    .variants
                    .iter()
                    .map(|variant| {
                        let mut props = serde_json::Map::new();

                        // Discriminator field with a const value.
                        props.insert(
                            tagged.discriminator.clone(),
                            json!({ "const": variant.tag }),
                        );

                        let mut required_names: Vec<Value> =
                            vec![json!(tagged.discriminator.clone())];

                        for field in &variant.fields {
                            props.insert(field.name.clone(), field_to_schema(field));
                            if field.required {
                                required_names.push(json!(field.name));
                            }
                        }

                        let mut variant_schema = serde_json::Map::new();
                        variant_schema.insert("type".to_string(), json!("object"));
                        variant_schema.insert("properties".to_string(), Value::Object(props));
                        variant_schema.insert("required".to_string(), Value::Array(required_names));
                        variant_schema.insert("additionalProperties".to_string(), json!(false));

                        Value::Object(variant_schema)
                    })
                    .collect();

                json!({ "oneOf": one_of })
            }
        },

        TypeDefKind::Alias(ty) => type_to_schema(ty),
    };

    if let Some(docs) = &def.docs
        && let Some(map) = schema.as_object_mut()
    {
        map.insert("description".to_string(), json!(docs));
    }

    schema
}

fn field_to_schema(field: &Field) -> Value {
    // Start with the core type schema.
    let mut schema = match &field.ty {
        // Optional<T> means the field may be absent; the type itself is T.
        Type::Optional(inner) => type_to_schema(inner),
        other => type_to_schema(other),
    };

    // nullable: true → wrap with anyOf [schema, {type: null}]
    if field.nullable {
        schema = json!({ "anyOf": [schema, { "type": "null" }] });
    }

    // Apply constraints.
    if let Some(c) = &field.constraints {
        apply_constraints(&mut schema, c);
    }

    // Default value.
    if let Some(default) = &field.default
        && let Some(map) = schema.as_object_mut()
    {
        map.insert("default".to_string(), default_value_to_json(default));
    }

    // Field-level doc comment.
    if let Some(docs) = &field.docs
        && let Some(map) = schema.as_object_mut()
    {
        map.insert("description".to_string(), json!(docs));
    }

    schema
}

fn type_to_schema(ty: &Type) -> Value {
    match ty {
        Type::String => json!({ "type": "string" }),
        Type::Integer { .. } => json!({ "type": "integer" }),
        Type::Float { .. } => json!({ "type": "number" }),
        Type::Boolean => json!({ "type": "boolean" }),
        Type::Null => json!({ "type": "null" }),
        Type::Any => json!({}),

        Type::Array(inner) => json!({
            "type": "array",
            "items": type_to_schema(inner)
        }),

        Type::Map { value, .. } => json!({
            "type": "object",
            "additionalProperties": type_to_schema(value)
        }),

        Type::Optional(inner) => {
            // Optional at the type level → anyOf with null
            json!({ "anyOf": [type_to_schema(inner), { "type": "null" }] })
        }

        Type::Ref(name) => json!({ "$ref": format!("#/$defs/{name}") }),

        Type::Union(types) => {
            let schemas: Vec<Value> = types.iter().map(type_to_schema).collect();
            json!({ "anyOf": schemas })
        }

        Type::StringLiteral(s) => json!({ "const": s }),
        Type::IntLiteral(n) => json!({ "const": n }),
        Type::BoolLiteral(b) => json!({ "const": b }),
    }
}

fn apply_constraints(schema: &mut Value, c: &FieldConstraints) {
    if let Some(map) = schema.as_object_mut() {
        if let Some(min) = c.min {
            map.insert("minimum".to_string(), json!(min));
        }
        if let Some(max) = c.max {
            map.insert("maximum".to_string(), json!(max));
        }
        if let Some(min_len) = c.min_length {
            map.insert("minLength".to_string(), json!(min_len));
        }
        if let Some(max_len) = c.max_length {
            map.insert("maxLength".to_string(), json!(max_len));
        }
        if let Some(pattern) = &c.pattern {
            map.insert("pattern".to_string(), json!(pattern));
        }
        if let Some(format) = &c.format {
            map.insert("format".to_string(), json!(format));
        }
    }
}

fn default_value_to_json(d: &DefaultValue) -> Value {
    match d {
        DefaultValue::String(s) => json!(s),
        DefaultValue::Number(n) => json!(n),
        DefaultValue::Bool(b) => json!(b),
        DefaultValue::Null => Value::Null,
    }
}

/// Static backend instance.
pub static JSON_SCHEMA_BACKEND: JsonSchemaBackend = JsonSchemaBackend;

/// JSON Schema output backend.
pub struct JsonSchemaBackend;

impl Backend for JsonSchemaBackend {
    fn name(&self) -> &'static str {
        "jsonschema"
    }

    fn language(&self) -> &'static str {
        "json"
    }

    fn extension(&self) -> &'static str {
        "json"
    }

    fn category(&self) -> BackendCategory {
        BackendCategory::Types
    }

    fn generate(&self, schema: &Schema) -> String {
        generate_json_schema(schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Field, FieldConstraints, TypeDef};

    fn parse_output(schema: &Schema) -> Value {
        let s = generate_json_schema(schema);
        serde_json::from_str(&s).expect("output is valid JSON")
    }

    #[test]
    fn simple_struct() {
        let mut schema = Schema::default();
        schema.add(TypeDef::structure(
            "User",
            vec![
                Field::required("id", Type::String),
                Field::optional("email", Type::String),
            ],
        ));

        let v = parse_output(&schema);
        let user = &v["$defs"]["User"];
        assert_eq!(user["type"], "object");
        assert_eq!(user["properties"]["id"]["type"], "string");
        let required = &user["required"];
        assert!(required.as_array().unwrap().contains(&json!("id")));
        assert!(!required.as_array().unwrap().contains(&json!("email")));
    }

    #[test]
    fn string_enum() {
        let mut schema = Schema::default();
        schema.add(TypeDef::string_enum("Status", vec!["active", "inactive"]));
        let v = parse_output(&schema);
        let status = &v["$defs"]["Status"];
        assert!(
            status["enum"]
                .as_array()
                .unwrap()
                .contains(&json!("active"))
        );
    }

    #[test]
    fn ref_type() {
        let mut schema = Schema::default();
        schema.add(TypeDef::string_enum("Status", vec!["active"]));
        schema.add(TypeDef::structure(
            "User",
            vec![Field::required("status", Type::Ref("Status".to_string()))],
        ));
        let v = parse_output(&schema);
        assert_eq!(
            v["$defs"]["User"]["properties"]["status"]["$ref"],
            "#/$defs/Status"
        );
    }

    #[test]
    fn nullable_field() {
        let mut schema = Schema::default();
        schema.add(TypeDef::structure(
            "Obj",
            vec![Field::required("value", Type::String).nullable()],
        ));
        let v = parse_output(&schema);
        let field = &v["$defs"]["Obj"]["properties"]["value"];
        assert!(field["anyOf"].is_array());
    }

    #[test]
    fn constraints() {
        let mut schema = Schema::default();
        schema.add(TypeDef::structure(
            "Obj",
            vec![
                Field::required("name", Type::String).with_constraints(FieldConstraints {
                    min_length: Some(1),
                    max_length: Some(100),
                    ..Default::default()
                }),
            ],
        ));
        let v = parse_output(&schema);
        let field = &v["$defs"]["Obj"]["properties"]["name"];
        assert_eq!(field["minLength"], 1);
        assert_eq!(field["maxLength"], 100);
    }
}
