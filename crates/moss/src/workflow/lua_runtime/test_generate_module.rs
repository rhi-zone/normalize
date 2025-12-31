//! Tests for the `type.generate` Lua module.

use super::LuaRuntime;
use std::path::Path;

#[test]
fn primitives() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        local generate = require("type.generate")

        math.randomseed(42)

        local s = generate(T.string)
        assert(type(s) == "string", "generated string should be string")

        local n = generate(T.number)
        assert(type(n) == "number", "generated number should be number")

        local i = generate(T.integer)
        assert(type(i) == "number" and i % 1 == 0, "generated integer should be whole")

        local b = generate(T.boolean)
        assert(type(b) == "boolean", "generated boolean should be boolean")
        "#,
    );
    assert!(result.is_ok(), "generate primitives failed: {:?}", result);
}

#[test]
fn with_constraints() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local generate = require("type.generate")
        math.randomseed(42)

        -- String with length constraints
        for i = 1, 10 do
            local s = generate({ type = "string", min_len = 5, max_len = 10 })
            assert(#s >= 5 and #s <= 10, "string length should be 5-10")
        end

        -- Number with range
        for i = 1, 10 do
            local n = generate({ type = "number", min = 0, max = 100 })
            assert(n >= 0 and n <= 100, "number should be 0-100")
        end

        -- one_of constraint
        for i = 1, 10 do
            local s = generate({ type = "string", one_of = { "red", "green", "blue" } })
            assert(s == "red" or s == "green" or s == "blue", "should be one of choices")
        end
        "#,
    );
    assert!(result.is_ok(), "generate constraints failed: {:?}", result);
}

#[test]
fn composite_types() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        local generate = require("type.generate")
        math.randomseed(42)

        -- Struct (use required fields for deterministic test)
        local s = generate(T.struct({
            name = { type = "string", required = true },
            age = { type = "integer", required = true }
        }))
        assert(type(s) == "table" and type(s.name) == "string", "struct generation")

        -- Array
        local a = generate(T.array(T.number), { max_array_len = 5 })
        assert(type(a) == "table", "array generation")

        -- Tuple
        local t = generate(T.tuple({ T.string, T.number }))
        assert(type(t[1]) == "string" and type(t[2]) == "number", "tuple generation")

        -- Literal
        assert(generate(T.literal("fixed")) == "fixed", "literal generation")
        "#,
    );
    assert!(result.is_ok(), "generate composite failed: {:?}", result);
}

#[test]
fn generated_values_validate() {
    let runtime = LuaRuntime::new(Path::new(".")).unwrap();
    let result = runtime.run_string(
        r#"
        local T = require("type")
        local validate = require("type.validate")
        local generate = require("type.generate")
        math.randomseed(42)

        local schema = T.struct({
            name = { type = "string", min_len = 1 },
            port = T.port,
            tags = T.array({ type = "string", one_of = { "a", "b", "c" } }),
        })

        for i = 1, 5 do
            local generated = generate(schema, { max_array_len = 3 })
            local _, err = validate.check(generated, schema)
            assert(err == nil, "generated value should validate: " .. tostring(err))
        end
        "#,
    );
    assert!(result.is_ok(), "generate validates failed: {:?}", result);
}
