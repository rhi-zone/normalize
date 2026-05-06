//! Protobuf IDL (`.proto`) to IR parser.
//!
//! Extracts `message` and `enum` declarations from Protobuf 3 (proto3) and
//! proto2 source files into the typegen IR.
//!
//! `service` blocks are not extracted — they carry RPC signatures rather than
//! data types and have no natural IR equivalent.
//!
//! # Parser notes
//!
//! This is a hand-rolled line-oriented parser. A tree-sitter grammar for
//! Protobuf is not available in arborium (arborium does not include one as of
//! 2026-05). Both proto2 and proto3 syntax are regular enough that a simple
//! state-machine parser covers the common cases.
//!
//! Limitations (acceptable for the common case):
//! - Multi-line string default values that span block boundaries are not parsed.
//! - `oneof` fields are flattened into the parent message as optional fields.
//! - Map fields (`map<K,V>`) are mapped to `Type::Map`.
//! - `extend` blocks are ignored.

use super::ParseError;
use crate::ir::{
    EnumDef, EnumKind, Field, IntVariant, Schema, StructDef, Type, TypeDef, TypeDefKind,
};

/// Parse a Protobuf IDL source string and extract type definitions into IR.
pub fn parse_proto(source: &str) -> Result<Schema, ParseError> {
    let mut parser = ProtoParser::new(source);
    parser.parse()
}

// ---------------------------------------------------------------------------
// Internal parser
// ---------------------------------------------------------------------------

struct ProtoParser<'a> {
    source: &'a str,
}

impl<'a> ProtoParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn parse(&mut self) -> Result<Schema, ParseError> {
        let mut schema = Schema::new();
        let tokens = tokenize(self.source);
        let mut pos = 0;

        while pos < tokens.len() {
            match tokens[pos].kind {
                TokenKind::Comment => {
                    pos += 1;
                }
                TokenKind::Word => {
                    let word = tokens[pos].text;
                    match word {
                        "syntax" | "package" | "option" | "import" | "service" | "extend" => {
                            // Skip to the next `;` or balanced `{...}` block
                            pos = skip_statement(&tokens, pos);
                        }
                        "message" => {
                            let (def, next) = parse_message(&tokens, pos, self.source)?;
                            schema.add(def);
                            pos = next;
                        }
                        "enum" => {
                            let (def, next) = parse_enum_def(&tokens, pos, self.source)?;
                            schema.add(def);
                            pos = next;
                        }
                        _ => {
                            pos += 1;
                        }
                    }
                }
                _ => {
                    pos += 1;
                }
            }
        }

        Ok(schema)
    }
}

// ---------------------------------------------------------------------------
// Token types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenKind {
    Word,
    Punct, // single characters: `{`, `}`, `;`, `=`, `<`, `>`, `,`, `[`, `]`
    Comment,
    StringLit,
    Number,
}

#[derive(Debug, Clone, Copy)]
struct Token<'a> {
    kind: TokenKind,
    text: &'a str,
}

/// Tokenize proto source into a flat list of tokens.
fn tokenize(source: &str) -> Vec<Token<'_>> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < len {
        // Skip whitespace
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Line comment `//`
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let start = i;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                text: &source[start..i],
            });
            continue;
        }

        // Block comment `/* ... */`
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2; // consume `*/`
            tokens.push(Token {
                kind: TokenKind::Comment,
                text: &source[start..i],
            });
            continue;
        }

        // String literal
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1; // skip escaped char
                }
                i += 1;
            }
            i += 1; // closing `"`
            tokens.push(Token {
                kind: TokenKind::StringLit,
                text: &source[start..i],
            });
            continue;
        }

        // Punctuation
        if matches!(
            bytes[i],
            b'{' | b'}' | b';' | b'=' | b'<' | b'>' | b',' | b'[' | b']' | b'(' | b')'
        ) {
            tokens.push(Token {
                kind: TokenKind::Punct,
                text: &source[i..i + 1],
            });
            i += 1;
            continue;
        }

        // Number (including negative and dotted floats)
        if bytes[i].is_ascii_digit() || bytes[i] == b'-' {
            let start = i;
            if bytes[i] == b'-' {
                i += 1;
            }
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                text: &source[start..i],
            });
            continue;
        }

        // Word (identifier or keyword)
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.' {
            let start = i;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Word,
                text: &source[start..i],
            });
            continue;
        }

        // Skip any other byte
        i += 1;
    }

    tokens
}

// ---------------------------------------------------------------------------
// Statement / block skipping
// ---------------------------------------------------------------------------

/// Skip a statement that ends with `;` or a balanced `{...}` block.
/// Returns the index of the token after the statement.
fn skip_statement(tokens: &[Token<'_>], start: usize) -> usize {
    let mut pos = start;
    while pos < tokens.len() {
        match tokens[pos].text {
            ";" => return pos + 1,
            "{" => {
                pos = skip_block(tokens, pos);
                return pos;
            }
            _ => pos += 1,
        }
    }
    tokens.len()
}

/// Skip a balanced `{...}` block starting at `{`.
/// Returns the index of the token after `}`.
fn skip_block(tokens: &[Token<'_>], open: usize) -> usize {
    debug_assert_eq!(tokens[open].text, "{");
    let mut depth = 0;
    let mut pos = open;
    while pos < tokens.len() {
        match tokens[pos].text {
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                if depth == 0 {
                    return pos + 1;
                }
            }
            _ => {}
        }
        pos += 1;
    }
    tokens.len()
}

// ---------------------------------------------------------------------------
// Comment collection
// ---------------------------------------------------------------------------

/// Collect the doc-comment that immediately precedes `pos`.
/// Returns the cleaned comment text, or `None` if no comment precedes.
fn collect_leading_comment<'a>(tokens: &[Token<'a>], pos: usize) -> Option<String> {
    if pos == 0 {
        return None;
    }
    let tok = &tokens[pos - 1];
    if tok.kind != TokenKind::Comment {
        return None;
    }
    Some(clean_comment(tok.text))
}

fn clean_comment(raw: &str) -> String {
    if let Some(inner) = raw.strip_prefix("//") {
        return inner.trim().to_string();
    }
    if let Some(inner) = raw.strip_prefix("/*").and_then(|s| s.strip_suffix("*/")) {
        return inner
            .lines()
            .map(|l| l.trim().trim_start_matches('*').trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    raw.trim().to_string()
}

// ---------------------------------------------------------------------------
// Message parsing
// ---------------------------------------------------------------------------

/// Parse `message Name { ... }` starting at the `message` keyword token.
/// Returns `(TypeDef, next_pos)`.
fn parse_message<'a>(
    tokens: &[Token<'a>],
    start: usize,
    _source: &str,
) -> Result<(TypeDef, usize), ParseError> {
    // tokens[start] == "message"
    let docs = collect_leading_comment(tokens, start);
    let mut pos = start + 1;

    // Name
    let name = expect_word(tokens, pos)?;
    pos += 1;

    // `{`
    expect_punct(tokens, pos, "{")?;
    pos += 1;

    let (fields, next) = parse_message_body(tokens, pos)?;
    let mut def = TypeDef {
        name: name.to_string(),
        docs,
        kind: TypeDefKind::Struct(StructDef { fields }),
    };
    // docs may have been set above; keep it
    let _ = &mut def;
    Ok((def, next))
}

/// Parse the body of a `message` up to and including the closing `}`.
/// Returns `(fields, next_pos)`.
fn parse_message_body(
    tokens: &[Token<'_>],
    start: usize,
) -> Result<(Vec<Field>, usize), ParseError> {
    let mut fields = Vec::new();
    let mut pos = start;

    while pos < tokens.len() {
        match tokens[pos].kind {
            TokenKind::Comment => {
                pos += 1;
            }
            TokenKind::Punct if tokens[pos].text == "}" => {
                return Ok((fields, pos + 1));
            }
            TokenKind::Word => {
                let word = tokens[pos].text;
                match word {
                    "message" => {
                        // Nested message — skip it (nested types aren't flattened to top-level)
                        pos = skip_statement(tokens, pos);
                    }
                    "enum" => {
                        // Nested enum — skip it
                        pos = skip_statement(tokens, pos);
                    }
                    "oneof" => {
                        // oneof Name { fields } — flatten fields as optional
                        let (oneof_fields, next) = parse_oneof(tokens, pos)?;
                        fields.extend(oneof_fields);
                        pos = next;
                    }
                    "option" | "extensions" | "reserved" => {
                        pos = skip_statement(tokens, pos);
                    }
                    "map" => {
                        // map<KeyType, ValueType> name = N;
                        let (field, next) = parse_map_field(tokens, pos)?;
                        fields.push(field);
                        pos = next;
                    }
                    _ => {
                        // Regular field: [modifier] type name = N [options];
                        match parse_field(tokens, pos) {
                            Ok((field, next)) => {
                                fields.push(field);
                                pos = next;
                            }
                            Err(_) => {
                                // Unknown construct — skip to next `;`
                                pos = skip_statement(tokens, pos);
                            }
                        }
                    }
                }
            }
            _ => {
                pos += 1;
            }
        }
    }

    Err(ParseError::Unsupported(
        "message body missing closing `}`".into(),
    ))
}

/// Parse `oneof name { field* }`.
fn parse_oneof(tokens: &[Token<'_>], start: usize) -> Result<(Vec<Field>, usize), ParseError> {
    // tokens[start] == "oneof"
    let mut pos = start + 1;
    // skip name
    pos += 1;
    // expect `{`
    expect_punct(tokens, pos, "{")?;
    pos += 1;
    let mut fields = Vec::new();
    while pos < tokens.len() {
        match tokens[pos].kind {
            TokenKind::Punct if tokens[pos].text == "}" => {
                return Ok((fields, pos + 1));
            }
            TokenKind::Comment => {
                pos += 1;
            }
            TokenKind::Word if tokens[pos].text == "option" => {
                pos = skip_statement(tokens, pos);
            }
            TokenKind::Word => {
                match parse_field(tokens, pos) {
                    Ok((mut field, next)) => {
                        // oneof fields are optional (at most one is set)
                        field.required = false;
                        fields.push(field);
                        pos = next;
                    }
                    Err(_) => {
                        pos = skip_statement(tokens, pos);
                    }
                }
            }
            _ => {
                pos += 1;
            }
        }
    }
    Err(ParseError::Unsupported("oneof body missing `}`".into()))
}

/// Parse a `map<KeyType, ValueType> field_name = N;` field.
fn parse_map_field(tokens: &[Token<'_>], start: usize) -> Result<(Field, usize), ParseError> {
    // tokens[start] == "map"
    let docs = collect_leading_comment(tokens, start);
    let mut pos = start + 1;

    expect_punct(tokens, pos, "<")?;
    pos += 1;

    let key_type_name = expect_word(tokens, pos)?;
    let key_ty = proto_scalar_type(key_type_name).unwrap_or(Type::Ref(key_type_name.to_string()));
    pos += 1;

    expect_punct(tokens, pos, ",")?;
    pos += 1;

    let val_type_name = expect_word(tokens, pos)?;
    let val_ty = proto_scalar_type(val_type_name).unwrap_or(Type::Ref(val_type_name.to_string()));
    pos += 1;

    expect_punct(tokens, pos, ">")?;
    pos += 1;

    let field_name = expect_word(tokens, pos)?;
    pos += 1;

    // = N
    expect_punct(tokens, pos, "=")?;
    pos += 1;
    // field number
    pos += 1;

    // skip optional `[options]`
    if pos < tokens.len() && tokens[pos].text == "[" {
        while pos < tokens.len() && tokens[pos].text != "]" {
            pos += 1;
        }
        pos += 1; // consume `]`
    }

    // `;`
    if pos < tokens.len() && tokens[pos].text == ";" {
        pos += 1;
    }

    let map_ty = Type::Map {
        key: Box::new(key_ty),
        value: Box::new(val_ty),
    };

    let mut field = Field::required(field_name.to_string(), map_ty);
    field.docs = docs;
    Ok((field, pos))
}

/// Parse a regular field: `[modifier] TypeName field_name = N [options];`
///
/// Modifiers: `optional`, `required`, `repeated`, `proto3_optional`
fn parse_field(tokens: &[Token<'_>], start: usize) -> Result<(Field, usize), ParseError> {
    let docs = collect_leading_comment(tokens, start);
    let mut pos = start;

    // Check for modifier
    let modifier = match tokens[pos].text {
        "optional" | "required" | "repeated" | "proto3_optional" => {
            let m = tokens[pos].text;
            pos += 1;
            m
        }
        _ => "",
    };

    // Type name (may be dotted: package.TypeName)
    let type_name = expect_word(tokens, pos)?;
    pos += 1;

    // Field name
    let field_name = expect_word(tokens, pos)?;
    pos += 1;

    // `= N`
    expect_punct(tokens, pos, "=")?;
    pos += 1;

    // field number (word or number token)
    if pos < tokens.len()
        && (tokens[pos].kind == TokenKind::Number || tokens[pos].kind == TokenKind::Word)
    {
        pos += 1;
    }

    // Skip optional `[options]`
    if pos < tokens.len() && tokens[pos].text == "[" {
        while pos < tokens.len() && tokens[pos].text != "]" {
            pos += 1;
        }
        if pos < tokens.len() {
            pos += 1; // consume `]`
        }
    }

    // `;`
    if pos < tokens.len() && tokens[pos].text == ";" {
        pos += 1;
    } else {
        return Err(ParseError::Unsupported(format!(
            "expected `;` after field `{}`",
            field_name
        )));
    }

    let base_ty = proto_scalar_type(type_name).unwrap_or(Type::Ref(type_name.to_string()));
    let ty = if modifier == "repeated" {
        Type::Array(Box::new(base_ty))
    } else {
        base_ty
    };

    let is_required = modifier != "optional" && modifier != "proto3_optional";
    let mut field = if is_required {
        Field::required(field_name.to_string(), ty)
    } else {
        Field::optional(field_name.to_string(), ty)
    };

    // In proto3 all scalar fields have implicit defaults — treat as required.
    // `optional` keyword explicitly marks a field as having presence (optional).
    field.docs = docs;
    Ok((field, pos))
}

// ---------------------------------------------------------------------------
// Enum parsing
// ---------------------------------------------------------------------------

/// Parse `enum Name { ... }` starting at the `enum` keyword token.
fn parse_enum_def<'a>(
    tokens: &[Token<'a>],
    start: usize,
    _source: &str,
) -> Result<(TypeDef, usize), ParseError> {
    let docs = collect_leading_comment(tokens, start);
    let mut pos = start + 1;

    let name = expect_word(tokens, pos)?;
    pos += 1;

    expect_punct(tokens, pos, "{")?;
    pos += 1;

    let mut variants: Vec<IntVariant> = Vec::new();

    while pos < tokens.len() {
        match tokens[pos].kind {
            TokenKind::Punct if tokens[pos].text == "}" => {
                pos += 1;
                break;
            }
            TokenKind::Comment => {
                pos += 1;
            }
            TokenKind::Word if tokens[pos].text == "option" => {
                pos = skip_statement(tokens, pos);
            }
            TokenKind::Word if tokens[pos].text == "reserved" => {
                pos = skip_statement(tokens, pos);
            }
            TokenKind::Word => {
                let variant_docs = collect_leading_comment(tokens, pos);
                let variant_name = tokens[pos].text;
                pos += 1;

                // `= N`
                expect_punct(tokens, pos, "=")?;
                pos += 1;

                // value (may be a negative number: `-1`)
                let value: i64 = if pos < tokens.len() {
                    match tokens[pos].text.parse::<i64>() {
                        Ok(v) => {
                            pos += 1;
                            v
                        }
                        Err(_) => {
                            pos += 1;
                            0
                        }
                    }
                } else {
                    0
                };

                // Skip optional `[options]`
                if pos < tokens.len() && tokens[pos].text == "[" {
                    while pos < tokens.len() && tokens[pos].text != "]" {
                        pos += 1;
                    }
                    if pos < tokens.len() {
                        pos += 1;
                    }
                }

                // `;`
                if pos < tokens.len() && tokens[pos].text == ";" {
                    pos += 1;
                }

                variants.push(IntVariant {
                    value,
                    name: Some(variant_name.to_string()),
                    docs: variant_docs,
                });
            }
            _ => {
                pos += 1;
            }
        }
    }

    let def = TypeDef {
        name: name.to_string(),
        docs,
        kind: TypeDefKind::Enum(EnumDef {
            kind: EnumKind::IntLiteral(variants),
        }),
    };
    Ok((def, pos))
}

// ---------------------------------------------------------------------------
// Scalar type mapping
// ---------------------------------------------------------------------------

/// Map Protobuf scalar type names to IR primitive types.
/// Returns `None` for unknown (non-scalar) type names.
fn proto_scalar_type(name: &str) -> Option<Type> {
    match name {
        "string" => Some(Type::String),
        "bytes" => Some(Type::Array(Box::new(Type::Integer {
            bits: 8,
            signed: false,
        }))),
        "bool" => Some(Type::Boolean),
        "float" => Some(Type::Float { bits: 32 }),
        "double" => Some(Type::Float { bits: 64 }),
        "int32" | "sint32" | "sfixed32" => Some(Type::Integer {
            bits: 32,
            signed: true,
        }),
        "int64" | "sint64" | "sfixed64" => Some(Type::Integer {
            bits: 64,
            signed: true,
        }),
        "uint32" | "fixed32" => Some(Type::Integer {
            bits: 32,
            signed: false,
        }),
        "uint64" | "fixed64" => Some(Type::Integer {
            bits: 64,
            signed: false,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Parser helpers
// ---------------------------------------------------------------------------

fn expect_word<'a>(tokens: &[Token<'a>], pos: usize) -> Result<&'a str, ParseError> {
    if pos >= tokens.len() {
        return Err(ParseError::Unsupported(
            "unexpected end of token stream (expected word)".into(),
        ));
    }
    if tokens[pos].kind != TokenKind::Word {
        return Err(ParseError::Unsupported(format!(
            "expected word, got `{}`",
            tokens[pos].text
        )));
    }
    Ok(tokens[pos].text)
}

fn expect_punct(tokens: &[Token<'_>], pos: usize, expected: &str) -> Result<(), ParseError> {
    if pos >= tokens.len() {
        return Err(ParseError::Unsupported(format!(
            "unexpected end of token stream (expected `{}`)",
            expected
        )));
    }
    if tokens[pos].text != expected {
        return Err(ParseError::Unsupported(format!(
            "expected `{}`, got `{}`",
            expected, tokens[pos].text
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EnumKind, Type, TypeDefKind};

    const SAMPLE_PROTO: &str = r#"
syntax = "proto3";

package example;

// A user in the system
message User {
  uint64 id = 1;
  string name = 2;
  string email = 3;
  repeated string tags = 4;
  optional Address address = 5;
}

message Address {
  string street = 1;
  string city = 2;
}

enum Status {
  // Unknown status
  UNKNOWN = 0;
  ACTIVE = 1;
  INACTIVE = 2;
}

service UserService {
  rpc GetUser (GetUserRequest) returns (User);
}

message GetUserRequest {
  uint64 id = 1;
}
"#;

    #[test]
    fn parse_proto_message() {
        let schema = parse_proto(SAMPLE_PROTO).expect("parse failed");
        let user = schema
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .expect("User not found");
        assert_eq!(user.docs.as_deref(), Some("A user in the system"));
        let TypeDefKind::Struct(s) = &user.kind else {
            panic!("expected struct");
        };
        assert_eq!(s.fields.len(), 5);

        // id: uint64 → required Integer{64, unsigned}
        let id_field = s.fields.iter().find(|f| f.name == "id").unwrap();
        assert!(id_field.required);
        assert!(matches!(
            id_field.ty,
            Type::Integer {
                bits: 64,
                signed: false
            }
        ));

        // tags: repeated string → Array(String)
        let tags_field = s.fields.iter().find(|f| f.name == "tags").unwrap();
        assert!(tags_field.required);
        assert!(matches!(tags_field.ty, Type::Array(_)));

        // address: optional Address → optional Ref
        let addr_field = s.fields.iter().find(|f| f.name == "address").unwrap();
        assert!(!addr_field.required);
        assert!(matches!(addr_field.ty, Type::Ref(_)));
    }

    #[test]
    fn parse_proto_enum() {
        let schema = parse_proto(SAMPLE_PROTO).expect("parse failed");
        let status = schema
            .definitions
            .iter()
            .find(|d| d.name == "Status")
            .expect("Status not found");
        let TypeDefKind::Enum(e) = &status.kind else {
            panic!("expected enum");
        };
        let EnumKind::IntLiteral(variants) = &e.kind else {
            panic!("expected int literal enum");
        };
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].name.as_deref(), Some("UNKNOWN"));
        assert_eq!(variants[0].value, 0);
        assert_eq!(variants[0].docs.as_deref(), Some("Unknown status"));
        assert_eq!(variants[1].name.as_deref(), Some("ACTIVE"));
        assert_eq!(variants[1].value, 1);
    }

    #[test]
    fn parse_proto_service_skipped() {
        let schema = parse_proto(SAMPLE_PROTO).expect("parse failed");
        // service UserService should NOT appear in the schema
        let has_service = schema.definitions.iter().any(|d| d.name == "UserService");
        assert!(!has_service);
    }

    #[test]
    fn parse_proto_type_count() {
        let schema = parse_proto(SAMPLE_PROTO).expect("parse failed");
        // User, Address, Status, GetUserRequest
        assert_eq!(schema.definitions.len(), 4);
    }

    #[test]
    fn parse_proto_map_field() {
        let proto = r#"
syntax = "proto3";
message Metadata {
  map<string, string> labels = 1;
  map<string, int32> counts = 2;
}
"#;
        let schema = parse_proto(proto).expect("parse failed");
        let meta = schema
            .definitions
            .iter()
            .find(|d| d.name == "Metadata")
            .unwrap();
        let TypeDefKind::Struct(s) = &meta.kind else {
            panic!("expected struct");
        };
        assert_eq!(s.fields.len(), 2);
        assert!(matches!(s.fields[0].ty, Type::Map { .. }));
    }

    #[test]
    fn parse_proto_oneof_fields() {
        let proto = r#"
syntax = "proto3";
message Request {
  string id = 1;
  oneof payload {
    string text = 2;
    bytes data = 3;
  }
}
"#;
        let schema = parse_proto(proto).expect("parse failed");
        let req = schema
            .definitions
            .iter()
            .find(|d| d.name == "Request")
            .unwrap();
        let TypeDefKind::Struct(s) = &req.kind else {
            panic!("expected struct");
        };
        // id + text + data = 3 fields (oneof flattened as optional)
        assert_eq!(s.fields.len(), 3);
        let text_field = s.fields.iter().find(|f| f.name == "text").unwrap();
        assert!(!text_field.required);
    }
}
