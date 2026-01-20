//! Integration tests for moss-codegen.

use rhizome_moss_codegen::{
    input::{parse_json_schema, parse_openapi},
    output::{
        go::{GoOptions, generate_go_types},
        pydantic::{PydanticOptions, generate_pydantic},
        python::{PythonOptions, PythonStyle, generate_python_types},
        rust::{RustOptions, generate_rust_types},
        typescript::{TypeScriptOptions, generate_typescript_types},
        valibot::{ValibotOptions, generate_valibot},
        zod::{ZodOptions, generate_zod},
    },
};

fn load_fixture(name: &str) -> serde_json::Value {
    let path = format!("tests/fixtures/{}.json", name);
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("fixture {} not found", name));
    serde_json::from_str(&content).expect("invalid JSON")
}

// === TypeScript Types ===

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

// === Zod ===

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

// === Valibot ===

#[test]
fn valibot_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_valibot(
        &schema,
        &ValibotOptions {
            export: true,
            infer_types: false,
        },
    );

    insta::assert_snapshot!(output);
}

#[test]
fn valibot_user_with_types() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_valibot(
        &schema,
        &ValibotOptions {
            export: true,
            infer_types: true,
        },
    );

    insta::assert_snapshot!(output);
}

// === Python Types ===

#[test]
fn python_dataclass_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_python_types(&schema, &PythonOptions::default());

    insta::assert_snapshot!(output);
}

#[test]
fn python_dataclass_frozen() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_python_types(
        &schema,
        &PythonOptions {
            frozen: true,
            ..Default::default()
        },
    );

    insta::assert_snapshot!(output);
}

#[test]
fn python_typeddict_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_python_types(
        &schema,
        &PythonOptions {
            style: PythonStyle::TypedDict,
            ..Default::default()
        },
    );

    insta::assert_snapshot!(output);
}

// === Pydantic ===

#[test]
fn pydantic_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_pydantic(&schema, &PydanticOptions::default());

    insta::assert_snapshot!(output);
}

#[test]
fn pydantic_user_frozen() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_pydantic(
        &schema,
        &PydanticOptions {
            frozen: true,
            ..Default::default()
        },
    );

    insta::assert_snapshot!(output);
}

// === Go ===

#[test]
fn go_types_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_go_types(&schema, &GoOptions::with_package("models"));

    insta::assert_snapshot!(output);
}

#[test]
fn go_types_no_tags() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_go_types(
        &schema,
        &GoOptions {
            package: "models".into(),
            json_tags: false,
            pointer_optionals: true,
            omitempty: false,
        },
    );

    insta::assert_snapshot!(output);
}

// === Rust ===

#[test]
fn rust_types_user() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_rust_types(&schema, &RustOptions::with_serde());

    insta::assert_snapshot!(output);
}

#[test]
fn rust_types_no_serde() {
    let input = load_fixture("user");
    let schema = parse_json_schema(&input).unwrap();
    let output = generate_rust_types(
        &schema,
        &RustOptions {
            debug: true,
            clone: true,
            partial_eq: true,
            public: true,
            ..Default::default()
        },
    );

    insta::assert_snapshot!(output);
}

// === OpenAPI Input ===

#[test]
fn openapi_petstore_typescript() {
    let input = load_fixture("petstore");
    let schema = parse_openapi(&input).unwrap();
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
fn openapi_petstore_zod() {
    let input = load_fixture("petstore");
    let schema = parse_openapi(&input).unwrap();
    let output = generate_zod(
        &schema,
        &ZodOptions {
            export: true,
            infer_types: true,
        },
    );

    insta::assert_snapshot!(output);
}
