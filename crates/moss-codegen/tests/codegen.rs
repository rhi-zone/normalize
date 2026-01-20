//! Integration tests for moss-codegen.

use rhizome_moss_codegen::{
    input::parse_json_schema,
    output::{
        typescript::{TypeScriptOptions, generate_typescript_types},
        zod::{ZodOptions, generate_zod},
    },
};

fn load_fixture(name: &str) -> serde_json::Value {
    let path = format!("tests/fixtures/{}.json", name);
    let content = std::fs::read_to_string(&path).expect(&format!("fixture {} not found", name));
    serde_json::from_str(&content).expect("invalid JSON")
}

#[test]
fn typescript_types_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_typescript_types(
        &schema,
        &TypeScriptOptions {
            export: true,
            ..Default::default()
        },
    );

    insta::assert_snapshot!(output);
}

#[test]
fn typescript_types_readonly() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_typescript_types(
        &schema,
        &TypeScriptOptions {
            export: true,
            readonly: true,
            ..Default::default()
        },
    );

    insta::assert_snapshot!(output);
}

#[test]
fn zod_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_zod(
        &schema,
        &ZodOptions {
            export: true,
            infer_types: false,
        },
    );

    insta::assert_snapshot!(output);
}

#[test]
fn zod_user_with_types() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_zod(
        &schema,
        &ZodOptions {
            export: true,
            infer_types: true,
        },
    );

    insta::assert_snapshot!(output);
}
