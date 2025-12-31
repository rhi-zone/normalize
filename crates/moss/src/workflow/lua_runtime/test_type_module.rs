//! Tests for the `type` Lua module.

use super::LuaRuntime;
use std::path::Path;

#[test]
fn primitives() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        assert(T.string.type == "string", "T.string")
        assert(T.number.type == "number", "T.number")
        assert(T.integer.type == "integer", "T.integer")
        assert(T.boolean.type == "boolean", "T.boolean")
        assert(T.any.type == "any", "T.any")
        assert(T["nil"].type == "nil", "T.nil")
        "#,
    );
    assert!(result.is_ok(), "type primitives failed: {:?}", result);
}

#[test]
fn constructors() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")

        local s = T.struct({ name = T.string })
        assert(s.type == "struct", "struct type")
        assert(s.shape.name.type == "string", "struct shape")

        local a = T.array(T.number)
        assert(a.type == "array", "array type")
        assert(a.item.type == "number", "array item")

        local o = T.optional(T.string)
        assert(o.type == "optional", "optional type")
        assert(o.inner.type == "string", "optional inner")

        local u = T.any_of(T.string, T.number)
        assert(u.type == "any_of", "any_of type")
        assert(#u.types == 2, "any_of types count")

        local l = T.literal("foo")
        assert(l.type == "literal", "literal type")
        assert(l.value == "foo", "literal value")

        local t = T.tuple({ T.string, T.number })
        assert(t.type == "tuple", "tuple type")
        assert(#t.shape == 2, "tuple shape count")

        local d = T.dictionary(T.string, T.number)
        assert(d.type == "dictionary", "dictionary type")
        assert(d.key.type == "string", "dictionary key")
        assert(d.value.type == "number", "dictionary value")

        local all = T.all_of(T.string, { type = "string", min_len = 1 })
        assert(all.type == "all_of", "all_of type")
        assert(#all.types == 2, "all_of types count")
        "#,
    );
    assert!(result.is_ok(), "type constructors failed: {:?}", result);
}

#[test]
fn aliases() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")

        assert(T.port.type == "integer", "port type")
        assert(T.port.min == 1, "port min")
        assert(T.port.max == 65535, "port max")

        assert(T.non_empty_string.type == "string", "non_empty_string type")
        assert(T.non_empty_string.min_len == 1, "non_empty_string min_len")

        assert(T.positive.type == "number", "positive type")
        assert(T.positive.min == 0, "positive min")
        assert(T.positive.exclusive_min == true, "positive exclusive_min")

        assert(T.non_negative.type == "number", "non_negative type")
        assert(T.non_negative.min == 0, "non_negative min")

        assert(T.file_exists.type == "string", "file_exists type")
        assert(T.file_exists.file_exists == true, "file_exists flag")

        assert(T.dir_exists.type == "string", "dir_exists type")
        assert(T.dir_exists.dir_exists == true, "dir_exists flag")
        "#,
    );
    assert!(result.is_ok(), "type aliases failed: {:?}", result);
}

#[test]
fn describe_primitives() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        local describe = require("type.describe")

        assert(describe(T.string) == "string", "describe string")
        assert(describe(T.number) == "number", "describe number")
        assert(describe(T.integer) == "integer", "describe integer")
        assert(describe(T.boolean) == "boolean", "describe boolean")
        assert(describe(T.any) == "any", "describe any")
        assert(describe(T["nil"]) == "nil", "describe nil")
        "#,
    );
    assert!(result.is_ok(), "describe primitives failed: {:?}", result);
}

#[test]
fn describe_with_constraints() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        local describe = require("type.describe")

        -- String constraints
        local s1 = describe({ type = "string", min_len = 1, max_len = 10 })
        assert(s1:find("1%-10 chars"), "string length range: " .. s1)

        local s2 = describe({ type = "string", pattern = "^[a-z]+$" })
        assert(s2:find("/^%[a%-z%]"), "string pattern: " .. s2)

        local s3 = describe({ type = "string", one_of = { "a", "b", "c" } })
        assert(s3:find("one of"), "string one_of: " .. s3)

        -- Number constraints
        local n1 = describe({ type = "number", min = 0, max = 100 })
        assert(n1:find(">= 0") and n1:find("<= 100"), "number range: " .. n1)

        local n2 = describe({ type = "number", min = 0, exclusive_min = true })
        assert(n2:find("> 0"), "number exclusive min: " .. n2)

        -- Port alias
        local p = describe(T.port)
        assert(p:find("integer") and p:find("1") and p:find("65535"), "port: " .. p)
        "#,
    );
    assert!(
        result.is_ok(),
        "describe with constraints failed: {:?}",
        result
    );
}

#[test]
fn describe_composite() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        local describe = require("type.describe")

        -- Array
        local a = describe(T.array(T.number))
        assert(a == "array of number", "array: " .. a)

        -- Optional
        local o = describe(T.optional(T.string))
        assert(o == "string?", "optional: " .. o)

        -- Tuple
        local t = describe(T.tuple({ T.string, T.number }))
        assert(t == "tuple(string, number)", "tuple: " .. t)

        -- Dictionary
        local d = describe(T.dictionary(T.string, T.number))
        assert(d == "dict<string, number>", "dictionary: " .. d)

        -- Union
        local u = describe(T.any_of(T.string, T.number))
        assert(u == "string | number", "any_of: " .. u)

        -- Intersection
        local i = describe(T.all_of(T.string, { type = "string", min_len = 1 }))
        assert(i:find("&"), "all_of: " .. i)

        -- Literal
        local l = describe(T.literal("foo"))
        assert(l == '"foo"', "literal string: " .. l)

        local ln = describe(T.literal(42))
        assert(ln == "42", "literal number: " .. ln)

        -- Struct
        local s = describe(T.struct({ name = T.string, age = T.integer }))
        assert(s:find("struct") and s:find("name") and s:find("age"), "struct: " .. s)
        "#,
    );
    assert!(result.is_ok(), "describe composite failed: {:?}", result);
}

#[test]
fn describe_custom_description() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        local describe = require("type.describe")

        -- Custom description takes priority
        local schema = { type = "string", description = "User's email address" }
        assert(describe(schema) == "User's email address", "custom description")

        -- Nested custom description
        local nested = T.struct({
            email = { type = "string", description = "Email address" },
            age = T.integer,
        })
        local desc = describe(nested)
        assert(desc:find("Email address"), "nested custom: " .. desc)
        "#,
    );
    assert!(
        result.is_ok(),
        "describe custom description failed: {:?}",
        result
    );
}
