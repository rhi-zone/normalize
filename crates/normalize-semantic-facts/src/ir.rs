//! The fact IR — a minimal, language-agnostic representation of structural
//! declarations (entity fields, enums, function signatures).
//!
//! Identity is equality on the IR: two source constructs that lower to the
//! same [`Fact`] value *are* the same fact, full stop. There is no fuzzy
//! matching layer on top. That means every normalization decision has to be
//! made once, in the extractor, at lowering time — not deferred to a
//! similarity score. This file documents the normalization choices the
//! prototype currently makes; they are deliberately simple and are expected
//! to be revisited (see `OVERVIEW.md`'s Open Questions).
//!
//! Normalization decisions baked into this IR today:
//! - **Entity and field names are normalized to space case.** Names are split
//!   on case boundaries (`camelCase`/`PascalCase`) and on underscores
//!   (`snake_case`/`SCREAMING_SNAKE_CASE`), lowercased, and rejoined with
//!   single spaces. This lets `interface Lesson` (TypeScript, PascalCase),
//!   `createdAt` (TypeScript, camelCase), and `created_at` (SQL, snake_case)
//!   all converge on a shared spelling (`created at`) without requiring the
//!   two languages to agree on a casing convention. It is a real information
//!   loss (a name that differs only in case/word-boundary elsewhere would
//!   incorrectly converge) — acceptable for a prototype, not for production
//!   identity.
//! - **Enum variants are sorted and deduplicated.** Source order in a SQL
//!   `CHECK (... IN (...))` list and a TypeScript union type carries no
//!   semantic weight, so two enums with the same variant set converge
//!   regardless of the order they were declared in.
//! - **Primitive type names are canonicalized per-extractor** into a shared
//!   vocabulary (`string`, `number`, `boolean`, ...) before reaching the IR.
//!   SQL's `TEXT` and TypeScript's `string` both lower to
//!   `TypeShape::Named("string")`. The mapping tables live next to each
//!   extractor since the source vocabulary is language-specific; the target
//!   vocabulary (the `Named` strings the IR accepts) is not yet a closed set
//!   — this is exactly the "how deep does type resolution go" open question.

use serde::{Deserialize, Serialize};

/// Configuration for entity-name canonicalization. Currently just a list of
/// suffixes to strip before space-casing — this is what lets a validator
/// schema variable named `NpsSurveyRowSchema` converge with a TypeScript
/// `interface NpsSurveyRow` under [`canonical_name`]: strip the `Schema`
/// suffix first, and the two names produce the same canonical spelling.
///
/// Stripping is repeated to a fixpoint, not just applied once — this
/// matters for layered names like `CreateDiscountInputSchema`, generated
/// from an authored `CreateDiscountInput` interface. Single-pass stripping
/// would remove only `Schema` (the first list match) from the generated
/// name, landing on `CreateDiscountInput`, while the interface's own name
/// strips `Input` down to `CreateDiscount` — two different canonical forms
/// for what's meant to be the same entity. Repeating the strip until no
/// suffix matches lands both on `CreateDiscount`. Confirmed against a real
/// codebase (`busiless`), not a hypothetical — see the task that added this
/// note.
///
/// Threaded through [`crate::FactExtractor::extract`] /
/// [`crate::extract_from_source`] rather than being a global — see
/// CLAUDE.md's "configuration flows in via constructors" rule. Construct
/// with [`NameConfig::default`] for the standard suffix list, or build a
/// custom one directly (all fields are `pub`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameConfig {
    /// Suffixes stripped from a name (exact case match, checked in list
    /// order, first match wins per pass, repeated to a fixpoint) before
    /// space-casing. A strip is only applied when the remainder would be
    /// non-empty, which also guarantees the loop terminates.
    pub strip_suffixes: Vec<String>,
}

impl Default for NameConfig {
    fn default() -> Self {
        NameConfig {
            strip_suffixes: [
                "Schema",
                "Validator",
                "Type",
                "Input",
                "Output",
                "Dto",
                "Model",
                "Props",
                "Params",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

/// Repeatedly strips the first configured suffix (in list order) that
/// `name` ends with, provided doing so leaves a non-empty remainder, until
/// no suffix matches. Returns `name` unchanged if no suffix ever matches.
/// Termination: each strip strictly shortens a non-empty string, so the
/// loop runs at most `name.len()` times.
fn strip_configured_suffix<'a>(name: &'a str, config: &NameConfig) -> &'a str {
    let mut current = name;
    loop {
        let Some(next) = strip_one_configured_suffix(current, config) else {
            return current;
        };
        current = next;
    }
}

fn strip_one_configured_suffix<'a>(name: &'a str, config: &NameConfig) -> Option<&'a str> {
    for suffix in &config.strip_suffixes {
        if let Some(stripped) = name.strip_suffix(suffix.as_str())
            && !stripped.is_empty()
        {
            return Some(stripped);
        }
    }
    None
}

/// Normalizes a name to "space case" for identity purposes: strip a
/// configured suffix (see [`NameConfig`]), split on case boundaries
/// (`camelCase`/`PascalCase`) and on underscores/hyphens
/// (`snake_case`/`SCREAMING_SNAKE_CASE`/`kebab-case`), lowercase each word,
/// and join with single spaces. See the module-level doc comment for why
/// this normalization exists and what it costs.
///
/// Examples: `createdAt`, `created_at`, `CreatedAt`, and `CREATED_AT` all
/// normalize to `"created at"`.
pub fn canonical_name(name: &str, config: &NameConfig) -> String {
    let name = strip_configured_suffix(name, config);
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' || c == '-' || c == ' ' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        if i > 0 && !current.is_empty() {
            let prev = chars[i - 1];
            // lower->upper: "createdAt" -> "created" | "At"
            let lower_to_upper = prev.is_lowercase() && c.is_uppercase();
            // end of an acronym run: "HTTPServer" -> "HTTP" | "Server"
            let acronym_to_word = prev.is_uppercase()
                && c.is_uppercase()
                && chars.get(i + 1).is_some_and(|next| next.is_lowercase());
            // letter/digit boundary: "field2" -> "field" | "2"
            let alnum_boundary = prev.is_ascii_digit() != c.is_ascii_digit();
            if lower_to_upper || acronym_to_word || alnum_boundary {
                words.push(std::mem::take(&mut current));
            }
        }
        current.push(c.to_ascii_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words.join(" ")
}

/// A normalized shape for a type. This is the recursive core of the fact IR:
/// entity fields, function parameters, and return types all resolve to one
/// of these.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeShape {
    /// A named/primitive type, canonicalized to a shared vocabulary
    /// (`"string"`, `"number"`, `"boolean"`, ...) by the extractor.
    Named(String),
    /// An inline enumeration — a closed set of string variants, sorted and
    /// deduplicated for identity. Distinct from [`Fact::EnumDef`], which is
    /// a *named* top-level enum declaration; this variant is what a field's
    /// type looks like when it's an inline union/CHECK-IN list rather than a
    /// reference to a declared enum.
    Enum(Vec<String>),
    /// An array/list of some element type.
    Array(Box<TypeShape>),
    /// A nullable/optional wrapper around some type.
    Optional(Box<TypeShape>),
    /// A compound/record type: an ordered list of (field name, field type)
    /// pairs, sorted by field name for identity.
    Record(Vec<(String, TypeShape)>),
}

impl TypeShape {
    /// Build a canonical [`TypeShape::Enum`] from an arbitrary iterator of
    /// variant strings: dedupes and sorts so that source order and
    /// duplication never affect identity.
    pub fn enum_of<I, S>(variants: I) -> TypeShape
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut variants: Vec<String> = variants.into_iter().map(Into::into).collect();
        variants.sort();
        variants.dedup();
        TypeShape::Enum(variants)
    }

    /// Build a canonical [`TypeShape::Record`], sorted by field name.
    pub fn record_of(mut fields: Vec<(String, TypeShape)>) -> TypeShape {
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        TypeShape::Record(fields)
    }
}

/// The relationship between two [`TypeShape`]s, as classified by
/// [`TypeShape::relate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeRelation {
    /// Identical shapes.
    Equal,
    /// `self <: other` (self is narrower/more specific).
    Subtype,
    /// `other <: self` (self is broader/more general).
    Supertype,
    /// The two shapes share structure (same variant, related substructure)
    /// but neither is a subtype of the other.
    Related,
    /// No structural relationship at all.
    Unrelated,
}

impl TypeShape {
    /// Structural subtyping: is `self` a subtype of `other`?
    ///
    /// Rules:
    /// - Reflexive: `T <: T`.
    /// - `T <: Optional(T)` — a non-nullable type is a subtype of its
    ///   nullable version.
    /// - `Optional(T) <: Optional(U)` if `T <: U` (covariant).
    /// - `Array(T) <: Array(U)` if `T <: U` (covariant).
    /// - `Enum(a) <: Enum(b)` if `a`'s variant set is a subset of `b`'s
    ///   (fewer variants = smaller, more specific set).
    pub fn is_subtype_of(&self, other: &TypeShape) -> bool {
        if self == other {
            return true;
        }
        match (self, other) {
            // Optional(T) <: Optional(U) if T <: U (covariant) — checked
            // before the general "T <: Optional(U)" fallback below so a
            // self that's already Optional recurses on its own inner type
            // rather than re-wrapping.
            (TypeShape::Optional(self_inner), TypeShape::Optional(other_inner)) => {
                self_inner.is_subtype_of(other_inner)
            }
            // T <: Optional(U) if T <: U (covers the T <: Optional(T) case
            // via reflexivity inside the recursive call).
            (_, TypeShape::Optional(other_inner)) => self.is_subtype_of(other_inner),
            (TypeShape::Array(self_inner), TypeShape::Array(other_inner)) => {
                self_inner.is_subtype_of(other_inner)
            }
            (TypeShape::Enum(a), TypeShape::Enum(b)) => {
                !a.is_empty() && a.iter().all(|v| b.contains(v))
            }
            _ => false,
        }
    }

    /// Classifies the relationship between `self` and `other`. See
    /// [`TypeRelation`].
    pub fn relate(&self, other: &TypeShape) -> TypeRelation {
        if self == other {
            return TypeRelation::Equal;
        }
        if self.is_subtype_of(other) {
            return TypeRelation::Subtype;
        }
        if other.is_subtype_of(self) {
            return TypeRelation::Supertype;
        }
        if Self::shares_structure(self, other) {
            return TypeRelation::Related;
        }
        TypeRelation::Unrelated
    }

    /// Whether `a` and `b` share enough structure to be worth reporting as
    /// `Related` even though neither is a subtype of the other — same
    /// top-level variant, or recursively related substructure (e.g.
    /// `Array(String)` vs bare `String`).
    fn shares_structure(a: &TypeShape, b: &TypeShape) -> bool {
        match (a, b) {
            (TypeShape::Named(x), TypeShape::Named(y)) => x == y,
            (TypeShape::Enum(x), TypeShape::Enum(y)) => x.iter().any(|v| y.contains(v)),
            (TypeShape::Array(x), TypeShape::Array(y)) => {
                x == y || !matches!(x.relate(y), TypeRelation::Unrelated)
            }
            (TypeShape::Optional(x), TypeShape::Optional(y)) => {
                x == y || !matches!(x.relate(y), TypeRelation::Unrelated)
            }
            (TypeShape::Record(x), TypeShape::Record(y)) => {
                x.iter().any(|(name, _)| y.iter().any(|(n, _)| n == name))
            }
            (TypeShape::Optional(inner), other) | (other, TypeShape::Optional(inner)) => {
                Self::shares_structure(inner, other)
            }
            (TypeShape::Array(inner), other) | (other, TypeShape::Array(inner)) => {
                Self::shares_structure(inner, other)
            }
            _ => false,
        }
    }
}

/// "Entity `X` has field `Y` of type `Z`."
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityField {
    /// Canonicalized entity/table/interface name.
    pub entity: String,
    /// Canonicalized field/column/property name.
    pub field: String,
    /// The field's normalized type.
    pub ty: TypeShape,
}

/// "Enum `X` has variants `[a, b, c]`." A *named*, top-level enum
/// declaration — as opposed to an inline `TypeShape::Enum` used as a field's
/// type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EnumDef {
    /// Canonicalized enum name.
    pub name: String,
    /// Sorted, deduplicated variant list.
    pub variants: Vec<String>,
}

/// "Function `X` takes `(a: T1, b: T2)` returns `T3`."
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionSignature {
    /// Canonicalized function/method name.
    pub name: String,
    /// Ordered (name, type) pairs. Parameter order is part of identity —
    /// unlike enum variants, reordering parameters changes the signature.
    pub params: Vec<(String, TypeShape)>,
    /// The normalized return type.
    pub returns: TypeShape,
}

/// A single semantic fact, lowered to the IR. Identity is `derive(PartialEq,
/// Eq, Hash)` — nothing fuzzier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Fact {
    EntityField(EntityField),
    EnumDef(EnumDef),
    FunctionSignature(FunctionSignature),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_name_converges_camel_snake_and_pascal() {
        let config = NameConfig::default();
        assert_eq!(canonical_name("createdAt", &config), "created at");
        assert_eq!(canonical_name("created_at", &config), "created at");
        assert_eq!(canonical_name("CreatedAt", &config), "created at");
        assert_eq!(canonical_name("CREATED_AT", &config), "created at");
        assert_eq!(canonical_name("partyRef", &config), "party ref");
        assert_eq!(canonical_name("party_ref", &config), "party ref");
        assert_eq!(
            canonical_name("push_subscriptions", &config),
            "push subscriptions"
        );
        assert_eq!(
            canonical_name("PushSubscriptions", &config),
            "push subscriptions"
        );
    }

    #[test]
    fn canonical_name_handles_acronyms_and_digits() {
        let config = NameConfig::default();
        assert_eq!(canonical_name("HTTPServer", &config), "http server");
        assert_eq!(canonical_name("field2", &config), "field 2");
        assert_eq!(canonical_name("kebab-case", &config), "kebab case");
        assert_eq!(canonical_name("already lower", &config), "already lower");
        assert_eq!(canonical_name("simple", &config), "simple");
    }

    #[test]
    fn canonical_name_strips_configured_suffix_before_space_casing() {
        let config = NameConfig::default();
        // Default suffix list includes "Schema" — this is what lets a
        // validator schema variable converge with a same-shape interface.
        assert_eq!(
            canonical_name("NpsSurveyRowSchema", &config),
            canonical_name("NpsSurveyRow", &config)
        );
        assert_eq!(
            canonical_name("NpsSurveyRowSchema", &config),
            "nps survey row"
        );
    }

    #[test]
    fn canonical_name_custom_suffix_list_changes_canonicalization() {
        let default_config = NameConfig::default();
        let custom_config = NameConfig {
            strip_suffixes: vec!["Row".to_string()],
        };
        // "Schema" is not in the custom list, so it survives space-casing;
        // "Row" is, so it gets stripped instead.
        assert_eq!(
            canonical_name("NpsSurveyRowSchema", &default_config),
            "nps survey row"
        );
        assert_eq!(canonical_name("NpsSurveyRow", &custom_config), "nps survey");
    }

    /// Suffix stripping repeats to a fixpoint, not just once — this is what
    /// lets a generated schema name like `CreateDiscountInputSchema`
    /// (`Schema` appended onto an already-`Input`-suffixed interface name)
    /// converge with the bare interface `CreateDiscountInput`, which itself
    /// strips down to `CreateDiscount`. Single-pass stripping would stop
    /// after removing `Schema`, landing on `CreateDiscountInput` — a
    /// different canonical form than the interface's own `CreateDiscount`.
    /// Found via a real `busiless` codegen pattern, not a hypothetical.
    #[test]
    fn canonical_name_strips_suffixes_repeatedly_to_a_fixpoint() {
        let config = NameConfig::default();
        assert_eq!(
            canonical_name("CreateDiscountInputSchema", &config),
            canonical_name("CreateDiscountInput", &config)
        );
        assert_eq!(
            canonical_name("CreateDiscountInputSchema", &config),
            "create discount"
        );
    }

    #[test]
    fn reflexive_subtyping() {
        let t = TypeShape::Named("string".to_string());
        assert!(t.is_subtype_of(&t));
        assert_eq!(t.relate(&t), TypeRelation::Equal);
    }

    #[test]
    fn non_nullable_is_subtype_of_optional() {
        let string_t = TypeShape::Named("string".to_string());
        let opt_string = TypeShape::Optional(Box::new(TypeShape::Named("string".to_string())));
        assert!(string_t.is_subtype_of(&opt_string));
        assert!(!opt_string.is_subtype_of(&string_t));
        assert_eq!(string_t.relate(&opt_string), TypeRelation::Subtype);
        assert_eq!(opt_string.relate(&string_t), TypeRelation::Supertype);
    }

    #[test]
    fn fewer_enum_variants_is_subtype() {
        let small = TypeShape::enum_of(["a", "b"]);
        let big = TypeShape::enum_of(["a", "b", "c"]);
        assert!(small.is_subtype_of(&big));
        assert!(!big.is_subtype_of(&small));
        assert_eq!(small.relate(&big), TypeRelation::Subtype);
        assert_eq!(big.relate(&small), TypeRelation::Supertype);
    }

    #[test]
    fn array_covariance() {
        let small = TypeShape::Array(Box::new(TypeShape::enum_of(["a", "b"])));
        let big = TypeShape::Array(Box::new(TypeShape::enum_of(["a", "b", "c"])));
        assert!(small.is_subtype_of(&big));
        assert!(!big.is_subtype_of(&small));
        assert_eq!(small.relate(&big), TypeRelation::Subtype);
    }

    #[test]
    fn optional_covariance() {
        let small = TypeShape::Optional(Box::new(TypeShape::enum_of(["a", "b"])));
        let big = TypeShape::Optional(Box::new(TypeShape::enum_of(["a", "b", "c"])));
        assert!(small.is_subtype_of(&big));
        assert_eq!(small.relate(&big), TypeRelation::Subtype);
    }

    #[test]
    fn overlapping_enums_are_related_not_subtype() {
        let a = TypeShape::enum_of(["a", "b"]);
        let b = TypeShape::enum_of(["b", "c"]);
        assert!(!a.is_subtype_of(&b));
        assert!(!b.is_subtype_of(&a));
        assert_eq!(a.relate(&b), TypeRelation::Related);
        assert_eq!(b.relate(&a), TypeRelation::Related);
    }

    #[test]
    fn array_of_string_vs_bare_string_is_related() {
        let array_string = TypeShape::Array(Box::new(TypeShape::Named("string".to_string())));
        let string_t = TypeShape::Named("string".to_string());
        assert!(!array_string.is_subtype_of(&string_t));
        assert!(!string_t.is_subtype_of(&array_string));
        assert_eq!(array_string.relate(&string_t), TypeRelation::Related);
        assert_eq!(string_t.relate(&array_string), TypeRelation::Related);
    }

    #[test]
    fn unrelated_named_types_are_unrelated_only_when_disjoint_variants() {
        // Two disjoint enums share no variants: unrelated.
        let a = TypeShape::enum_of(["a", "b"]);
        let c = TypeShape::enum_of(["c", "d"]);
        assert_eq!(a.relate(&c), TypeRelation::Unrelated);
    }

    #[test]
    fn different_named_primitives_are_unrelated() {
        // Different primitive names (e.g. "string" vs "number") share no
        // structure — genuinely unrelated, not merely non-subtype.
        let string_t = TypeShape::Named("string".to_string());
        let number_t = TypeShape::Named("number".to_string());
        assert_eq!(string_t.relate(&number_t), TypeRelation::Unrelated);
    }
}
