//! Prototype: language-agnostic semantic fact IR and cross-language
//! restatement detection. See `OVERVIEW.md` for the problem statement and
//! open questions — this crate proves the core thesis (structurally
//! equivalent declarations in different languages lower to the same fact
//! IR node) and nothing more. No persistence, no CLI, no query files yet.

pub mod extract;
pub mod ir;
pub mod restatement;
pub mod sql;
pub mod typescript;

pub use extract::{FactExtractor, FactOccurrence};
pub use ir::{EntityField, EnumDef, Fact, FunctionSignature, TypeRelation, TypeShape};
pub use restatement::{
    Location, RestatementGroup, SimilarEntry, SimilarGroup, SimilarRelation, find_restatements,
    find_similar, restated_only,
};
pub use sql::SqlExtractor;
pub use typescript::TypeScriptExtractor;

/// Parses `source` with the grammar `extractor.grammar_name()` (via
/// `normalize_languages`'s shared `GrammarLoader` singleton) and runs the
/// extractor over the resulting tree. Returns `None` if the grammar can't be
/// loaded or the source fails to parse.
pub fn extract_from_source(
    extractor: &dyn FactExtractor,
    source: &str,
    file: &str,
) -> Option<Vec<FactOccurrence>> {
    let tree = normalize_languages::parsers::parse_with_grammar(extractor.grammar_name(), source)?;
    Some(extractor.extract(&tree, source, file))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TS_SOURCE: &str = r#"
type LessonStatus = "scheduled" | "in_progress" | "completed" | "cancelled";

interface Lesson {
  id: string;
  status: LessonStatus;
  title?: string;
  tags: string[];
}

function cancelLesson(id: string, reason: string): boolean {
  return true;
}
"#;

    const SQL_SOURCE: &str = r#"
CREATE TABLE lesson (
  id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('scheduled', 'in_progress', 'completed', 'cancelled')),
  title TEXT,
  tags TEXT NOT NULL
);
"#;

    fn ts_facts() -> Vec<FactOccurrence> {
        extract_from_source(&TypeScriptExtractor, TS_SOURCE, "src/types.ts")
            .expect("typescript grammar should load and parse")
    }

    fn sql_facts() -> Vec<FactOccurrence> {
        extract_from_source(&SqlExtractor, SQL_SOURCE, "migrations/001.sql")
            .expect("sql grammar should load and parse")
    }

    #[test]
    fn typescript_extracts_entity_fields_enum_and_function() {
        let facts: Vec<Fact> = ts_facts().into_iter().map(|o| o.fact).collect();

        assert!(facts.contains(&Fact::EnumDef(EnumDef {
            name: "lesson status".to_string(),
            variants: vec![
                "cancelled".to_string(),
                "completed".to_string(),
                "in_progress".to_string(),
                "scheduled".to_string(),
            ],
        })));

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "id".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "status".to_string(),
            ty: TypeShape::enum_of(["scheduled", "in_progress", "completed", "cancelled",]),
        })));

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "title".to_string(),
            ty: TypeShape::Optional(Box::new(TypeShape::Named("string".to_string()))),
        })));

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "tags".to_string(),
            ty: TypeShape::Array(Box::new(TypeShape::Named("string".to_string()))),
        })));

        assert!(facts.contains(&Fact::FunctionSignature(FunctionSignature {
            name: "cancel lesson".to_string(),
            params: vec![
                ("id".to_string(), TypeShape::Named("string".to_string())),
                ("reason".to_string(), TypeShape::Named("string".to_string())),
            ],
            returns: TypeShape::Named("boolean".to_string()),
        })));
    }

    #[test]
    fn sql_extracts_entity_fields_with_check_in_as_enum() {
        let facts: Vec<Fact> = sql_facts().into_iter().map(|o| o.fact).collect();

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "id".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "status".to_string(),
            ty: TypeShape::enum_of(["scheduled", "in_progress", "completed", "cancelled",]),
        })));

        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "title".to_string(),
            ty: TypeShape::Optional(Box::new(TypeShape::Named("string".to_string()))),
        })));

        // `tags` is a plain NOT NULL TEXT column in SQL, not an array — the
        // IR correctly does NOT converge this with TypeScript's
        // `tags: string[]`. Structurally different things should stay
        // distinct facts.
        assert!(facts.contains(&Fact::EntityField(EntityField {
            entity: "lesson".to_string(),
            field: "tags".to_string(),
            ty: TypeShape::Named("string".to_string()),
        })));
    }

    /// The core thesis: a TypeScript `interface` field and a SQL column
    /// definition that describe the same entity produce byte-identical fact
    /// IR nodes, and the restatement finder groups them together across the
    /// two source files/languages.
    #[test]
    fn cross_language_restatement_finds_identical_facts() {
        let mut occurrences = ts_facts();
        occurrences.extend(sql_facts());

        let groups = find_restatements(&occurrences);
        let restated = restated_only(groups);

        // Three fields are stated identically in both languages: id,
        // status (as an enum, via TS alias resolution / SQL CHECK IN), and
        // title (as optional/nullable). `tags` is NOT among them, because
        // TypeScript models it as an array and SQL models it as a plain
        // string column — genuinely different facts, correctly kept apart.
        assert_eq!(
            restated.len(),
            3,
            "expected exactly 3 restated facts, got: {restated:#?}"
        );

        let id_group = restated
            .iter()
            .find(|g| {
                matches!(&g.fact, Fact::EntityField(f) if f.entity == "lesson" && f.field == "id")
            })
            .expect("id field should be restated across TS and SQL");
        assert_eq!(id_group.count(), 2);
        assert!(id_group.locations.iter().any(|l| l.file == "src/types.ts"));
        assert!(
            id_group
                .locations
                .iter()
                .any(|l| l.file == "migrations/001.sql")
        );

        let status_group = restated
            .iter()
            .find(|g| {
                matches!(&g.fact, Fact::EntityField(f) if f.entity == "lesson" && f.field == "status")
            })
            .expect("status field should be restated across TS and SQL");
        assert_eq!(status_group.count(), 2);
        assert!(matches!(
            &status_group.fact,
            Fact::EntityField(f) if matches!(&f.ty, TypeShape::Enum(v) if v.len() == 4)
        ));

        let title_group = restated
            .iter()
            .find(|g| {
                matches!(&g.fact, Fact::EntityField(f) if f.entity == "lesson" && f.field == "title")
            })
            .expect("title field should be restated across TS and SQL");
        assert_eq!(title_group.count(), 2);
        assert!(matches!(
            &title_group.fact,
            Fact::EntityField(f) if matches!(&f.ty, TypeShape::Optional(inner) if **inner == TypeShape::Named("string".to_string()))
        ));
    }

    /// Approximate mode (`find_similar`) groups facts by identity
    /// (entity+field) alone, regardless of type. Unlike exact mode, the
    /// TS/SQL `tags` field — `Array(String)` vs bare `String`, which exact
    /// mode correctly keeps apart as distinct facts — now shows up in the
    /// same group with its `TypeRelation` classified, instead of being
    /// silently ungrouped.
    #[test]
    fn approximate_mode_relates_tags_field_across_languages() {
        let mut occurrences = ts_facts();
        occurrences.extend(sql_facts());

        let groups = find_similar(&occurrences);

        let tags_group = groups
            .iter()
            .find(|g| {
                g.entries.iter().any(|e| {
                    matches!(&e.fact, Fact::EntityField(f) if f.entity == "lesson" && f.field == "tags")
                })
            })
            .expect("lesson.tags should be grouped by approximate mode");

        // Two distinct type shapes: Array(String) from TS, Named(String) from SQL.
        assert_eq!(tags_group.entries.len(), 2);
        assert_eq!(tags_group.total_count(), 2);
        assert_eq!(tags_group.relations.len(), 1);
        assert_eq!(tags_group.relations[0].relation, TypeRelation::Related);
    }

    /// Exact mode (`find_restatements`) is unchanged by the addition of
    /// approximate mode: `tags` still does not restate across TS and SQL,
    /// because the two type shapes are genuinely different facts under IR
    /// equality.
    #[test]
    fn exact_mode_still_keeps_tags_field_apart() {
        let mut occurrences = ts_facts();
        occurrences.extend(sql_facts());

        let restated = restated_only(find_restatements(&occurrences));

        assert!(
            !restated.iter().any(|g| {
                matches!(&g.fact, Fact::EntityField(f) if f.entity == "lesson" && f.field == "tags")
            }),
            "tags should not be restated under exact IR equality"
        );
    }
}
