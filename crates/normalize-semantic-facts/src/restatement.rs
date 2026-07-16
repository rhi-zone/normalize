//! Groups identical facts (by IR equality) across all extracted occurrences
//! to find restatements — the same fact expressed more than once, possibly
//! in different languages or syntactic forms.
//!
//! Two modes, following `normalize-code-similarity`'s pattern of
//! user-selectable modes rather than tiers:
//! - [`find_restatements`] (exact) — groups by full IR equality. A `tags`
//!   field typed `Array(String)` in one place and `Named("string")` in
//!   another are *different facts* and never group together.
//! - [`find_similar`] (approximate) — groups by fact *identity* (same
//!   entity+field, enum name, or function name) regardless of type, then
//!   classifies how the type shapes relate via [`TypeShape::relate`]. This
//!   is what surfaces the `tags` case above as a `Related` (or `Subtype`/
//!   `Supertype`) pair instead of leaving it silently ungrouped.

use std::collections::HashMap;

use crate::extract::FactOccurrence;
use crate::ir::{Fact, TypeRelation, TypeShape};

/// Where a fact occurred: a file and 1-based line number.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Location {
    pub file: String,
    pub line: usize,
}

/// One distinct fact and every location it was found at.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RestatementGroup {
    pub fact: Fact,
    pub locations: Vec<Location>,
}

impl RestatementGroup {
    /// Number of times this fact was stated. `1` means it appears exactly
    /// once (not a restatement); `2+` means the same fact was independently
    /// declared in multiple places.
    pub fn count(&self) -> usize {
        self.locations.len()
    }
}

/// Groups `occurrences` by fact identity (IR equality) and returns one
/// [`RestatementGroup`] per distinct fact, sorted by restatement count
/// descending — the biggest compression targets first, matching
/// `OVERVIEW.md`'s output ordering.
pub fn find_restatements(occurrences: &[FactOccurrence]) -> Vec<RestatementGroup> {
    let mut groups: HashMap<&Fact, Vec<Location>> = HashMap::new();
    for occ in occurrences {
        groups.entry(&occ.fact).or_default().push(Location {
            file: occ.file.clone(),
            line: occ.line,
        });
    }

    let mut result: Vec<RestatementGroup> = groups
        .into_iter()
        .map(|(fact, locations)| RestatementGroup {
            fact: fact.clone(),
            locations,
        })
        .collect();

    result.sort_by(|a, b| {
        b.count()
            .cmp(&a.count())
            .then_with(|| format!("{:?}", a.fact).cmp(&format!("{:?}", b.fact)))
    });
    result
}

/// Convenience filter: only groups where the fact was actually restated
/// (found in more than one location).
pub fn restated_only(groups: Vec<RestatementGroup>) -> Vec<RestatementGroup> {
    groups.into_iter().filter(|g| g.count() > 1).collect()
}

/// A stable identity key for grouping facts loosely in [`find_similar`]:
/// same entity+field (or function/enum name), regardless of whether the
/// declared type matches. This is deliberately coarser than [`Fact`]'s own
/// `PartialEq`/`Hash` (full structural equality) — it is what lets a field
/// restated with a *different* type still be found and compared, instead of
/// silently falling into two unrelated groups the way [`find_restatements`]
/// would leave them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FactKey {
    EntityField { entity: String, field: String },
    EnumDef { name: String },
    FunctionSignature { name: String },
}

fn fact_key(fact: &Fact) -> FactKey {
    match fact {
        Fact::EntityField(f) => FactKey::EntityField {
            entity: f.entity.clone(),
            field: f.field.clone(),
        },
        Fact::EnumDef(e) => FactKey::EnumDef {
            name: e.name.clone(),
        },
        Fact::FunctionSignature(s) => FactKey::FunctionSignature {
            name: s.name.clone(),
        },
    }
}

/// The [`TypeShape`] a fact contributes for [`TypeShape::relate`]
/// comparisons in [`find_similar`]. An `EnumDef`'s variant list becomes an
/// inline `TypeShape::Enum`; a function signature's params + return type
/// become a synthetic `TypeShape::Record` (keyed `"return"` for the return
/// type) so a changed parameter or return type still produces a classified
/// relation instead of being silently ignored.
fn fact_type_shape(fact: &Fact) -> TypeShape {
    match fact {
        Fact::EntityField(f) => f.ty.clone(),
        Fact::EnumDef(e) => TypeShape::Enum(e.variants.clone()),
        Fact::FunctionSignature(s) => {
            let mut fields = s.params.clone();
            fields.push(("return".to_string(), s.returns.clone()));
            TypeShape::record_of(fields)
        }
    }
}

/// One distinct fact within a [`SimilarGroup`] and every location it was
/// found at — like [`RestatementGroup`], but a `SimilarGroup` may contain
/// several of these with *different* type shapes under the same identity
/// key.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SimilarEntry {
    pub fact: Fact,
    pub locations: Vec<Location>,
}

impl SimilarEntry {
    pub fn count(&self) -> usize {
        self.locations.len()
    }
}

/// The classified [`TypeRelation`] between two entries in a
/// [`SimilarGroup`], identified by their index into `SimilarGroup::entries`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SimilarRelation {
    pub a: usize,
    pub b: usize,
    pub relation: TypeRelation,
}

/// Every distinct fact sharing one identity key (same entity+field, enum
/// name, or function name), plus the pairwise [`TypeRelation`] between each
/// pair of distinct type shapes found under that key.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SimilarGroup {
    pub entries: Vec<SimilarEntry>,
    /// Pairwise relations between `entries`. Empty when the group has only
    /// one distinct type shape (nothing to relate) — such groups still
    /// appear in the output for parity with [`find_restatements`].
    pub relations: Vec<SimilarRelation>,
}

impl SimilarGroup {
    /// Total occurrences across every entry in the group.
    pub fn total_count(&self) -> usize {
        self.entries.iter().map(SimilarEntry::count).sum()
    }
}

/// Groups `occurrences` by fact identity key (same entity+field, enum name,
/// or function name — see [`FactKey`]) rather than requiring full IR
/// equality, then classifies the [`TypeRelation`] between every pair of
/// distinct type shapes within each group.
///
/// Complements [`find_restatements`]: that function requires exact IR
/// equality, so `tags: string[]` (TypeScript) and `tags TEXT` (SQL) are
/// correctly kept as two separate, unrelated facts. `find_similar` groups
/// them under the same `lesson.tags` identity key and reports the
/// `TypeRelation` between `Array(Named("string"))` and `Named("string")` —
/// `Related`, since one wraps the other's element type — instead of leaving
/// the near-miss invisible.
pub fn find_similar(occurrences: &[FactOccurrence]) -> Vec<SimilarGroup> {
    let mut groups: HashMap<FactKey, Vec<&FactOccurrence>> = HashMap::new();
    for occ in occurrences {
        groups.entry(fact_key(&occ.fact)).or_default().push(occ);
    }

    let mut result: Vec<SimilarGroup> = groups
        .into_values()
        .map(|occs| {
            // Sub-group by exact fact identity first (mirrors
            // find_restatements), so each entry represents one distinct
            // type shape within this identity key.
            let mut by_fact: HashMap<&Fact, Vec<Location>> = HashMap::new();
            for occ in &occs {
                by_fact.entry(&occ.fact).or_default().push(Location {
                    file: occ.file.clone(),
                    line: occ.line,
                });
            }
            let mut entries: Vec<SimilarEntry> = by_fact
                .into_iter()
                .map(|(fact, locations)| SimilarEntry {
                    fact: fact.clone(),
                    locations,
                })
                .collect();
            entries.sort_by(|a, b| format!("{:?}", a.fact).cmp(&format!("{:?}", b.fact)));

            let mut relations = Vec::new();
            for i in 0..entries.len() {
                for j in (i + 1)..entries.len() {
                    let a_ty = fact_type_shape(&entries[i].fact);
                    let b_ty = fact_type_shape(&entries[j].fact);
                    relations.push(SimilarRelation {
                        a: i,
                        b: j,
                        relation: a_ty.relate(&b_ty),
                    });
                }
            }

            SimilarGroup { entries, relations }
        })
        .collect();

    result.sort_by(|a, b| {
        b.total_count()
            .cmp(&a.total_count())
            .then_with(|| format!("{:?}", a.entries).cmp(&format!("{:?}", b.entries)))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occ(fact: Fact, file: &str, line: usize) -> FactOccurrence {
        FactOccurrence {
            fact,
            file: file.to_string(),
            line,
        }
    }

    #[test]
    fn find_similar_groups_by_identity_and_relates_types() {
        use crate::ir::EntityField;

        let occurrences = vec![
            occ(
                Fact::EntityField(EntityField {
                    entity: "lesson".to_string(),
                    field: "tags".to_string(),
                    ty: TypeShape::Array(Box::new(TypeShape::Named("string".to_string()))),
                }),
                "src/types.ts",
                4,
            ),
            occ(
                Fact::EntityField(EntityField {
                    entity: "lesson".to_string(),
                    field: "tags".to_string(),
                    ty: TypeShape::Named("string".to_string()),
                }),
                "migrations/001.sql",
                6,
            ),
        ];

        let groups = find_similar(&occurrences);
        assert_eq!(groups.len(), 1, "expected one group keyed on lesson.tags");
        let group = &groups[0];
        assert_eq!(group.entries.len(), 2, "two distinct type shapes");
        assert_eq!(group.relations.len(), 1);
        assert_eq!(group.relations[0].relation, TypeRelation::Related);
    }

    #[test]
    fn find_similar_single_shape_group_has_no_relations() {
        use crate::ir::EntityField;

        let occurrences = vec![
            occ(
                Fact::EntityField(EntityField {
                    entity: "lesson".to_string(),
                    field: "id".to_string(),
                    ty: TypeShape::Named("string".to_string()),
                }),
                "src/types.ts",
                3,
            ),
            occ(
                Fact::EntityField(EntityField {
                    entity: "lesson".to_string(),
                    field: "id".to_string(),
                    ty: TypeShape::Named("string".to_string()),
                }),
                "migrations/001.sql",
                2,
            ),
        ];

        let groups = find_similar(&occurrences);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].entries.len(), 1);
        assert!(groups[0].relations.is_empty());
        assert_eq!(groups[0].total_count(), 2);
    }
}
