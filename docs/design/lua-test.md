# Lua Test Library Design

Minimal test framework for moss scripts.

## Goals

1. Simple test organization with pass/fail tracking
2. Rich assertion library
3. Property-based testing via `type.generate` integration
4. No external dependencies

## Module Structure

```
test.lua           -- Core test runner and assertions
test/property.lua  -- Property-based testing
```

## Basic Usage

### Running Tests

```lua
local test = require("test")

test.test("addition works", function()
    test.assert.equals(2 + 2, 4)
end)

test.test("strings concatenate", function()
    test.assert.equals("hello" .. " world", "hello world")
end)

test.report()  -- prints summary, returns true if all passed
```

Output:
```
All 2 tests passed
```

### Test Lifecycle

```lua
test.reset()   -- clear pass/fail counts (optional, for multiple test runs)
-- run tests...
test.report()  -- print summary and return success boolean
```

## Assertions

All assertions take an optional `msg` parameter for custom error messages.

### Equality

```lua
test.assert.equals(a, b, msg)     -- a == b (shallow)
test.assert.same(a, b, msg)       -- deep equality for tables
```

### Truthiness

```lua
test.assert.is_true(v, msg)       -- v is truthy
test.assert.is_false(v, msg)      -- v is falsy
test.assert.is_nil(v, msg)        -- v == nil
test.assert.is_not_nil(v, msg)    -- v ~= nil
```

### Types

```lua
test.assert.is_type(v, "string", msg)   -- type(v) == expected
```

### Strings

```lua
test.assert.contains(str, substr, msg)  -- string.find(str, substr)
test.assert.matches(str, pattern, msg)  -- string.match(str, pattern)
```

### Collections

```lua
test.assert.is_in(value, tbl, msg)      -- value exists in table
```

### Errors

```lua
test.assert.throws(fn, pattern, msg)        -- fn() errors, message matches pattern
test.assert.does_not_throw(fn, msg)         -- fn() succeeds
```

### Numeric Comparisons

```lua
test.assert.near(a, b, tolerance, msg)  -- |a - b| <= tolerance
test.assert.gt(a, b, msg)               -- a > b
test.assert.gte(a, b, msg)              -- a >= b
test.assert.lt(a, b, msg)               -- a < b
test.assert.lte(a, b, msg)              -- a <= b
```

## Property-Based Testing

The `test.property` module generates random values from type schemas and verifies properties hold for all of them.

### Basic Property Check

```lua
local property = require("test.property")
local T = require("type")

-- Check that string length is non-negative
local ok, err = property.check(
    T.string({ min_len = 0, max_len = 100 }),
    function(s)
        assert(#s >= 0)
    end
)

if not ok then
    print("Property failed: " .. err)
end
```

### Integration with Test Module

Use `property.prop()` to create a test function:

```lua
local test = require("test")
local property = require("test.property")
local T = require("type")

test.test("integers are numbers", property.prop(
    "integer check",
    T.integer({ min = -100, max = 100 }),
    function(n)
        assert(type(n) == "number")
    end
))

test.report()
```

### Immediate Assertion

Use `property.assert()` for inline property checks:

```lua
local property = require("test.property")
local T = require("type")

property.assert(
    T.array(T.integer()),
    function(arr)
        -- array length is non-negative
        assert(#arr >= 0)
    end,
    { iterations = 50 }
)
```

### Options

```lua
property.check(schema, fn, {
    iterations = 100,  -- number of random values to test (default: 100)
    seed = 12345,      -- random seed for reproducibility
})
```

### Struct Properties

```lua
local user_schema = T.struct({
    name = T.string({ min_len = 1, max_len = 50 }),
    age = T.integer({ min = 0, max = 150 }),
})

property.assert(user_schema, function(user)
    assert(#user.name >= 1)
    assert(user.age >= 0)
end)
```

### Failure Reporting

When a property fails, the error message includes:
- Which iteration failed
- The generated value that caused failure
- The original error message

```
Property failed on iteration 42 with value: { name = "x", age = -5 }
Error: assertion failed: age must be non-negative
```

## Full Example

```lua
local test = require("test")
local property = require("test.property")
local T = require("type")

-- Unit tests
test.test("empty table has zero length", function()
    test.assert.equals(#{}, 0)
end)

test.test("table insert increases length", function()
    local t = {1, 2, 3}
    table.insert(t, 4)
    test.assert.equals(#t, 4)
end)

-- Property tests
test.test("string reversal is involutory", property.prop(
    "reverse twice equals original",
    T.string({ min_len = 0, max_len = 20 }),
    function(s)
        local reversed = s:reverse():reverse()
        assert(reversed == s)
    end,
    { iterations = 50 }
))

test.test("array concat preserves elements", property.prop(
    "concat length",
    T.struct({
        a = T.array(T.integer()),
        b = T.array(T.integer()),
    }),
    function(input)
        local combined = {}
        for _, v in ipairs(input.a) do table.insert(combined, v) end
        for _, v in ipairs(input.b) do table.insert(combined, v) end
        assert(#combined == #input.a + #input.b)
    end
))

-- Report results
local success = test.report()
os.exit(success and 0 or 1)
```

## Design Decisions

1. **No test discovery**: Tests run inline as the file executes. Simple and predictable.

2. **Assertions on `test.assert`**: Namespaced to avoid polluting global `assert`. The global `assert` still works for quick checks.

3. **Property testing uses type schemas**: Leverages `type.generate` for random value generation. No separate shrinking—failed cases show the exact value.

4. **No beforeEach/afterEach**: Keep it simple. Use local functions if setup is needed.

5. **Exit code convention**: `test.report()` returns boolean for use with `os.exit()`.
