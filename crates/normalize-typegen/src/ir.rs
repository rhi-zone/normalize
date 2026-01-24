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
    /// Whether the field is required.
    pub required: bool,
    /// Documentation comment.
    pub docs: Option<String>,
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

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, def: TypeDef) {
        self.definitions.push(def);
    }
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
            docs: None,
        }
    }

    pub fn optional(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
            required: false,
            docs: None,
        }
    }

    pub fn with_docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
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
}
