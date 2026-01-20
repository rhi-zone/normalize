//! OpenAPI to IR parser.
//!
//! Extracts type definitions from OpenAPI 3.x documents.
//! Uses the JSON Schema parser for the actual schema conversion.

use super::jsonschema::{ParseError, parse_json_schema};
use crate::ir::Schema;
use serde_json::Value;

/// Parse an OpenAPI 3.x document into an IR Schema.
///
/// Extracts schemas from:
/// - `#/components/schemas/*`
/// - Inline schemas in request/response bodies (flattened)
pub fn parse_openapi(input: &Value) -> Result<Schema, ParseError> {
    let mut schema = Schema::new();

    // Check OpenAPI version
    let version = input.get("openapi").and_then(|v| v.as_str()).unwrap_or("");
    if !version.starts_with("3.") {
        return Err(ParseError::Unsupported(format!(
            "OpenAPI version {} not supported (expected 3.x)",
            version
        )));
    }

    // Extract schemas from components/schemas
    if let Some(schemas) = input.pointer("/components/schemas") {
        // Convert OpenAPI structure to JSON Schema structure for reuse
        let json_schema = serde_json::json!({
            "$defs": schemas
        });
        let parsed = parse_json_schema(&json_schema)?;
        for def in parsed.definitions {
            schema.add(def);
        }
    }

    // Extract inline schemas from paths (request/response bodies)
    if let Some(paths) = input.get("paths").and_then(|p| p.as_object()) {
        for (_path, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (_method, operation) in methods_obj {
                    extract_inline_schemas(&mut schema, operation)?;
                }
            }
        }
    }

    Ok(schema)
}

/// Extract inline schemas from an operation's request/response bodies.
fn extract_inline_schemas(schema: &mut Schema, operation: &Value) -> Result<(), ParseError> {
    // Request body
    if let Some(request_body) = operation.get("requestBody") {
        extract_from_content(schema, request_body)?;
    }

    // Responses
    if let Some(responses) = operation.get("responses").and_then(|r| r.as_object()) {
        for (_status, response) in responses {
            extract_from_content(schema, response)?;
        }
    }

    Ok(())
}

/// Extract schemas from content media types.
fn extract_from_content(schema: &mut Schema, container: &Value) -> Result<(), ParseError> {
    if let Some(content) = container.get("content").and_then(|c| c.as_object()) {
        for (_media_type, media) in content {
            if let Some(inline_schema) = media.get("schema") {
                // Only process if it has a title (named type) and is an object
                if let Some(title) = inline_schema.get("title").and_then(|t| t.as_str()) {
                    // Check if this is a new type definition (not a $ref)
                    if inline_schema.get("$ref").is_none() {
                        let json_schema = serde_json::json!({
                            "title": title,
                            "type": inline_schema.get("type"),
                            "properties": inline_schema.get("properties"),
                            "required": inline_schema.get("required"),
                            "description": inline_schema.get("description")
                        });
                        let parsed = parse_json_schema(&json_schema)?;
                        for def in parsed.definitions {
                            // Avoid duplicates
                            if !schema.definitions.iter().any(|d| d.name == def.name) {
                                schema.add(def);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::TypeDefKind;
    use serde_json::json;

    #[test]
    fn parse_openapi_components() {
        let input = json!({
            "openapi": "3.0.3",
            "info": { "title": "Test API", "version": "1.0.0" },
            "paths": {},
            "components": {
                "schemas": {
                    "User": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "name": { "type": "string" }
                        },
                        "required": ["id", "name"]
                    },
                    "Status": {
                        "type": "string",
                        "enum": ["active", "inactive"]
                    }
                }
            }
        });

        let schema = parse_openapi(&input).unwrap();
        assert_eq!(schema.definitions.len(), 2);

        let user = schema
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .unwrap();
        assert!(matches!(&user.kind, TypeDefKind::Struct(_)));

        let status = schema
            .definitions
            .iter()
            .find(|d| d.name == "Status")
            .unwrap();
        assert!(matches!(&status.kind, TypeDefKind::Enum(_)));
    }

    #[test]
    fn parse_openapi_with_refs() {
        let input = json!({
            "openapi": "3.0.3",
            "info": { "title": "Test API", "version": "1.0.0" },
            "paths": {},
            "components": {
                "schemas": {
                    "Address": {
                        "type": "object",
                        "properties": {
                            "street": { "type": "string" }
                        }
                    },
                    "User": {
                        "type": "object",
                        "properties": {
                            "address": { "$ref": "#/components/schemas/Address" }
                        }
                    }
                }
            }
        });

        let schema = parse_openapi(&input).unwrap();
        assert_eq!(schema.definitions.len(), 2);
    }

    #[test]
    fn reject_openapi_2() {
        let input = json!({
            "swagger": "2.0",
            "info": { "title": "Test API", "version": "1.0.0" }
        });

        let result = parse_openapi(&input);
        assert!(result.is_err());
    }
}
