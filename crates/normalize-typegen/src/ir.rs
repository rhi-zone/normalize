//! Intermediate representation for type definitions.
//!
//! All input formats (JSON Schema, OpenAPI, Protobuf) normalize to this IR
//! before being passed to output backends.

use serde::{Deserialize, Serialize};

/// A complete schema containing multiple type definitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schema {
    /// All type definitions in the schema.
    pub definitions: Vec<TypeDef>,
}

/// A default value for a field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DefaultValue {
    /// A string default.
    String(String),
    /// A numeric default (stored as f64 for generality).
    Number(f64),
    /// A boolean default.
    Bool(bool),
    /// A null default.
    Null,
}

/// Constraints for field validation (min/max, length, pattern, format).
///
/// All fields are optional — only set the ones relevant to the field type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldConstraints {
    /// Minimum value (inclusive) for numeric fields.
    pub min: Option<f64>,
    /// Maximum value (inclusive) for numeric fields.
    pub max: Option<f64>,
    /// Minimum length for string or array fields.
    pub min_length: Option<u64>,
    /// Maximum length for string or array fields.
    pub max_length: Option<u64>,
    /// Regex pattern for string fields.
    pub pattern: Option<String>,
    /// Semantic format hint (e.g. "email", "uri", "date-time").
    pub format: Option<String>,
}

/// A named type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    /// Type name (e.g., "User", "OrderStatus").
    pub name: String,
    /// Documentation comment.
    pub docs: Option<String>,
    /// The type's shape.
    pub kind: TypeDefKind,
}

/// The kind of type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeDefKind {
    /// A struct with named fields.
    Struct(StructDef),
    /// An enum with variants.
    Enum(EnumDef),
    /// A type alias (e.g., `type UserId = string`).
    Alias(Type),
}

/// A struct definition with named fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructDef {
    pub fields: Vec<Field>,
}

/// A field in a struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    /// Field name as it appears in the schema.
    pub name: String,
    /// Field type.
    pub ty: Type,
    /// Whether the field is required (absent → field may be omitted).
    pub required: bool,
    /// Whether the field may hold an explicit `null` value in addition to its type.
    ///
    /// Distinct from `required`: a required nullable field must be present but may be `null`.
    /// An optional non-nullable field may be absent but, when present, must not be `null`.
    pub nullable: bool,
    /// Documentation comment.
    pub docs: Option<String>,
    /// Default value for the field (used by validators and documentation generators).
    pub default: Option<DefaultValue>,
    /// Validation constraints for the field.
    pub constraints: Option<FieldConstraints>,
}

/// An enum definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    /// The kind of enum.
    pub kind: EnumKind,
}

/// The kind of enum (string literals vs tagged union).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnumKind {
    /// String literal enum (e.g., "pending" | "active" | "done").
    StringLiteral(Vec<StringVariant>),
    /// Integer enum (e.g., 0 | 1 | 2).
    IntLiteral(Vec<IntVariant>),
    /// Tagged union / discriminated union.
    Tagged(TaggedUnion),
}

/// A string enum variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringVariant {
    /// The string value.
    pub value: String,
    /// Documentation comment.
    pub docs: Option<String>,
}

/// An integer enum variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntVariant {
    /// The integer value.
    pub value: i64,
    /// Optional name for the variant.
    pub name: Option<String>,
    /// Documentation comment.
    pub docs: Option<String>,
}

/// A tagged union (discriminated union).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggedUnion {
    /// The discriminator field name.
    pub discriminator: String,
    /// The variants.
    pub variants: Vec<TaggedVariant>,
}

/// A variant in a tagged union.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggedVariant {
    /// The discriminator value for this variant.
    pub tag: String,
    /// The variant's fields (excluding discriminator).
    pub fields: Vec<Field>,
    /// Documentation comment.
    pub docs: Option<String>,
}

/// A type reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Type {
    // Primitives
    String,
    Integer { bits: u8, signed: bool },
    Float { bits: u8 },
    Boolean,
    Null,

    // Compound
    Array(Box<Type>),
    Map { key: Box<Type>, value: Box<Type> },
    Optional(Box<Type>),

    // Reference to another type definition
    Ref(String),

    // Union (for anyOf/oneOf without discriminator)
    Union(Vec<Type>),

    // Literal types
    StringLiteral(String),
    IntLiteral(i64),
    BoolLiteral(bool),

    // Escape hatch
    Any,
}

/// Errors returned by [`Schema::validate`].
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// A type name is not a valid identifier.
    InvalidTypeName(String),
    /// A field name is not a valid identifier.
    InvalidFieldName {
        type_name: String,
        field_name: String,
    },
    /// Two definitions share the same name.
    DuplicateTypeName(String),
    /// Two fields in the same struct share the same name.
    DuplicateFieldName {
        type_name: String,
        field_name: String,
    },
    /// A `Ref` points to a type name that does not exist in this schema.
    UnresolvedRef { from: String, to: String },
    /// The schema contains a circular reference (type A → B → … → A).
    CircularRef(Vec<String>),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTypeName(n) => write!(f, "invalid type name: {n:?}"),
            Self::InvalidFieldName {
                type_name,
                field_name,
            } => {
                write!(f, "invalid field name {field_name:?} in type {type_name:?}")
            }
            Self::DuplicateTypeName(n) => write!(f, "duplicate type name: {n:?}"),
            Self::DuplicateFieldName {
                type_name,
                field_name,
            } => {
                write!(f, "duplicate field {field_name:?} in type {type_name:?}")
            }
            Self::UnresolvedRef { from, to } => {
                write!(f, "unresolved ref to {to:?} in type {from:?}")
            }
            Self::CircularRef(cycle) => write!(f, "circular reference: {}", cycle.join(" → ")),
        }
    }
}

impl std::error::Error for ValidationError {}

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, def: TypeDef) {
        self.definitions.push(def);
    }

    /// Validate the schema for well-formedness.
    ///
    /// Checks:
    /// - All type names and field names are valid identifiers (non-empty, start with a letter or
    ///   `_`, contain only alphanumerics, `_`, or `.`).
    /// - No duplicate type names.
    /// - No duplicate field names within a struct.
    /// - All `Ref` targets resolve to a defined type.
    /// - No circular references between types.
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Collect known type names for ref-resolution.
        let mut known: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut seen_names: std::collections::HashSet<&str> = std::collections::HashSet::new();

        for def in &self.definitions {
            if !is_valid_identifier(&def.name) {
                errors.push(ValidationError::InvalidTypeName(def.name.clone()));
            }
            if !seen_names.insert(def.name.as_str()) {
                errors.push(ValidationError::DuplicateTypeName(def.name.clone()));
            }
            known.insert(def.name.as_str());
        }

        // Per-type checks (field names, duplicate fields, unresolved refs).
        for def in &self.definitions {
            match &def.kind {
                TypeDefKind::Struct(s) => {
                    let mut seen_fields: std::collections::HashSet<&str> =
                        std::collections::HashSet::new();
                    for field in &s.fields {
                        if !is_valid_identifier(&field.name) {
                            errors.push(ValidationError::InvalidFieldName {
                                type_name: def.name.clone(),
                                field_name: field.name.clone(),
                            });
                        }
                        if !seen_fields.insert(field.name.as_str()) {
                            errors.push(ValidationError::DuplicateFieldName {
                                type_name: def.name.clone(),
                                field_name: field.name.clone(),
                            });
                        }
                        collect_unresolved_refs(&field.ty, &def.name, &known, &mut errors);
                    }
                }
                TypeDefKind::Enum(e) => {
                    if let EnumKind::Tagged(tagged) = &e.kind {
                        for variant in &tagged.variants {
                            for field in &variant.fields {
                                collect_unresolved_refs(&field.ty, &def.name, &known, &mut errors);
                            }
                        }
                    }
                }
                TypeDefKind::Alias(ty) => {
                    collect_unresolved_refs(ty, &def.name, &known, &mut errors);
                }
            }
        }

        // Circular reference detection via DFS.
        let adj = build_ref_graph(self);
        let names: Vec<&str> = self.definitions.iter().map(|d| d.name.as_str()).collect();
        let mut state: std::collections::HashMap<&str, DfsState> = std::collections::HashMap::new();
        for name in &names {
            if !state.contains_key(name) {
                let mut path = Vec::new();
                dfs_cycle(name, &adj, &mut state, &mut path, &mut errors);
            }
        }

        errors
    }
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

fn collect_unresolved_refs(
    ty: &Type,
    type_name: &str,
    known: &std::collections::HashSet<&str>,
    errors: &mut Vec<ValidationError>,
) {
    match ty {
        Type::Ref(name) if !known.contains(name.as_str()) => {
            errors.push(ValidationError::UnresolvedRef {
                from: type_name.to_string(),
                to: name.clone(),
            });
        }
        Type::Array(inner) | Type::Optional(inner) => {
            collect_unresolved_refs(inner, type_name, known, errors);
        }
        Type::Map { key, value } => {
            collect_unresolved_refs(key, type_name, known, errors);
            collect_unresolved_refs(value, type_name, known, errors);
        }
        Type::Union(types) => {
            for t in types {
                collect_unresolved_refs(t, type_name, known, errors);
            }
        }
        _ => {}
    }
}

fn build_ref_graph(schema: &Schema) -> std::collections::HashMap<String, Vec<String>> {
    let mut adj: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for def in &schema.definitions {
        let mut refs = Vec::new();
        collect_type_refs_for_def(def, &mut refs);
        // Deduplicate.
        refs.sort();
        refs.dedup();
        adj.insert(def.name.clone(), refs);
    }
    adj
}

fn collect_type_refs_for_def(def: &TypeDef, refs: &mut Vec<String>) {
    match &def.kind {
        TypeDefKind::Struct(s) => {
            for field in &s.fields {
                collect_type_refs_from_type(&field.ty, refs);
            }
        }
        TypeDefKind::Enum(e) => {
            if let EnumKind::Tagged(tagged) = &e.kind {
                for variant in &tagged.variants {
                    for field in &variant.fields {
                        collect_type_refs_from_type(&field.ty, refs);
                    }
                }
            }
        }
        TypeDefKind::Alias(ty) => collect_type_refs_from_type(ty, refs),
    }
}

fn collect_type_refs_from_type(ty: &Type, refs: &mut Vec<String>) {
    match ty {
        Type::Ref(name) => refs.push(name.clone()),
        Type::Array(inner) | Type::Optional(inner) => collect_type_refs_from_type(inner, refs),
        Type::Map { key, value } => {
            collect_type_refs_from_type(key, refs);
            collect_type_refs_from_type(value, refs);
        }
        Type::Union(types) => {
            for t in types {
                collect_type_refs_from_type(t, refs);
            }
        }
        _ => {}
    }
}

#[derive(PartialEq)]
enum DfsState {
    InStack,
    Done,
}

fn dfs_cycle<'a>(
    node: &'a str,
    adj: &'a std::collections::HashMap<String, Vec<String>>,
    state: &mut std::collections::HashMap<&'a str, DfsState>,
    path: &mut Vec<&'a str>,
    errors: &mut Vec<ValidationError>,
) {
    state.insert(node, DfsState::InStack);
    path.push(node);

    if let Some(neighbors) = adj.get(node) {
        for neighbor in neighbors {
            let neighbor_str: &str = neighbor.as_str();
            // We need a longer lifetime — map neighbor to a key from adj.
            if let Some(key) = adj.get_key_value(neighbor_str).map(|(k, _)| k.as_str()) {
                match state.get(key) {
                    Some(DfsState::InStack) => {
                        // Found cycle — report from where it starts.
                        let cycle_start = path.iter().position(|&n| n == key).unwrap_or(0);
                        let mut cycle: Vec<String> =
                            path[cycle_start..].iter().map(|s| s.to_string()).collect();
                        cycle.push(key.to_string()); // Close the loop.
                        errors.push(ValidationError::CircularRef(cycle));
                    }
                    Some(DfsState::Done) => {}
                    None => {
                        dfs_cycle(key, adj, state, path, errors);
                    }
                }
            }
        }
    }

    path.pop();
    state.insert(node, DfsState::Done);
}

impl TypeDef {
    pub fn structure(name: impl Into<String>, fields: Vec<Field>) -> Self {
        Self {
            name: name.into(),
            docs: None,
            kind: TypeDefKind::Struct(StructDef { fields }),
        }
    }

    pub fn string_enum(name: impl Into<String>, values: Vec<&str>) -> Self {
        Self {
            name: name.into(),
            docs: None,
            kind: TypeDefKind::Enum(EnumDef {
                kind: EnumKind::StringLiteral(
                    values
                        .into_iter()
                        .map(|v| StringVariant {
                            value: v.to_string(),
                            docs: None,
                        })
                        .collect(),
                ),
            }),
        }
    }

    pub fn with_docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }
}

impl Field {
    pub fn required(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
            required: true,
            nullable: false,
            docs: None,
            default: None,
            constraints: None,
        }
    }

    pub fn optional(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
            required: false,
            nullable: false,
            docs: None,
            default: None,
            constraints: None,
        }
    }

    pub fn with_docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    /// Mark the field as nullable (may hold an explicit `null` in addition to its declared type).
    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    /// Set a default value for the field.
    pub fn with_default(mut self, default: DefaultValue) -> Self {
        self.default = Some(default);
        self
    }

    /// Set validation constraints for the field.
    pub fn with_constraints(mut self, constraints: FieldConstraints) -> Self {
        self.constraints = Some(constraints);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_schema_programmatically() {
        let mut schema = Schema::new();

        schema.add(TypeDef::string_enum(
            "Status",
            vec!["pending", "active", "done"],
        ));

        schema.add(TypeDef::structure(
            "User",
            vec![
                Field::required("id", Type::String),
                Field::required("name", Type::String),
                Field::optional("email", Type::String),
                Field::required("status", Type::Ref("Status".into())),
            ],
        ));

        assert_eq!(schema.definitions.len(), 2);
    }

    #[test]
    fn field_nullable_and_default() {
        let f = Field::required("value", Type::String)
            .nullable()
            .with_default(DefaultValue::String("hello".to_string()))
            .with_constraints(FieldConstraints {
                min_length: Some(1),
                max_length: Some(255),
                pattern: Some(r"^\w+$".to_string()),
                format: Some("email".to_string()),
                ..Default::default()
            });

        assert!(f.nullable);
        assert_eq!(f.default, Some(DefaultValue::String("hello".to_string())));
        assert_eq!(f.constraints.as_ref().unwrap().min_length, Some(1));
        assert_eq!(f.constraints.as_ref().unwrap().max_length, Some(255));
    }

    #[test]
    fn validate_valid_schema() {
        let mut schema = Schema::new();
        schema.add(TypeDef::string_enum("Status", vec!["active", "inactive"]));
        schema.add(TypeDef::structure(
            "User",
            vec![
                Field::required("id", Type::String),
                Field::required("status", Type::Ref("Status".to_string())),
            ],
        ));
        let errors = schema.validate();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn validate_duplicate_type_name() {
        let mut schema = Schema::new();
        schema.add(TypeDef::string_enum("Status", vec!["a"]));
        schema.add(TypeDef::string_enum("Status", vec!["b"]));
        let errors = schema.validate();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::DuplicateTypeName(n) if n == "Status"))
        );
    }

    #[test]
    fn validate_unresolved_ref() {
        let mut schema = Schema::new();
        schema.add(TypeDef::structure(
            "User",
            vec![Field::required("role", Type::Ref("Role".to_string()))],
        ));
        let errors = schema.validate();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::UnresolvedRef { to, .. } if to == "Role"
        )));
    }

    #[test]
    fn validate_circular_ref() {
        let mut schema = Schema::new();
        schema.add(TypeDef::structure(
            "A",
            vec![Field::required("b", Type::Ref("B".to_string()))],
        ));
        schema.add(TypeDef::structure(
            "B",
            vec![Field::required("a", Type::Ref("A".to_string()))],
        ));
        let errors = schema.validate();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::CircularRef(_))),
            "expected circular ref error, got: {errors:?}"
        );
    }

    #[test]
    fn validate_invalid_identifier() {
        let mut schema = Schema::new();
        schema.add(TypeDef::structure("123Bad", vec![]));
        let errors = schema.validate();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::InvalidTypeName(n) if n == "123Bad"))
        );
    }
}
