-- Type definitions for declarative schemas
-- Usage: local T = require("type")

local M = {}

-- Primitive type constants

M.string = { type = "string" }
M.number = { type = "number" }
M.integer = { type = "integer" }
M.boolean = { type = "boolean" }
M.any = { type = "any" }
M["nil"] = { type = "nil" }

-- Shorthand constructors (plain functions returning tables)

function M.struct(shape)
    return { type = "struct", shape = shape }
end

function M.array(item)
    return { type = "array", item = item }
end

function M.optional(inner)
    return { type = "optional", inner = inner }
end

function M.any_of(...)
    return { type = "any_of", types = { ... } }
end

function M.all_of(...)
    return { type = "all_of", types = { ... } }
end

function M.literal(value)
    return { type = "literal", value = value }
end

function M.tuple(shape)
    return { type = "tuple", shape = shape }
end

function M.dictionary(key, value)
    return { type = "dictionary", key = key, value = value }
end

-- Built-in type aliases

M.file_exists = {
    type = "string",
    file_exists = true,
}

M.dir_exists = {
    type = "string",
    dir_exists = true,
}

M.port = { type = "integer", min = 1, max = 65535 }
M.positive = { type = "number", min = 0, exclusive_min = true }
M.non_negative = { type = "number", min = 0 }
M.non_empty_string = { type = "string", min_len = 1 }

-- Schema introspection

local describers = {}

--- Generate a human-readable description of a schema
--- @param schema table
--- @return string
function M.describe(schema)
    -- Custom description takes priority
    if schema.description then
        return schema.description
    end

    local describer = describers[schema.type]
    if describer then
        return describer(schema)
    end

    return schema.type or "unknown"
end

describers.string = function(schema)
    local parts = { "string" }
    if schema.min_len and schema.max_len then
        table.insert(parts, string.format("(%d-%d chars)", schema.min_len, schema.max_len))
    elseif schema.min_len then
        table.insert(parts, string.format("(min %d chars)", schema.min_len))
    elseif schema.max_len then
        table.insert(parts, string.format("(max %d chars)", schema.max_len))
    end
    if schema.pattern then
        table.insert(parts, string.format("matching /%s/", schema.pattern))
    end
    if schema.one_of then
        table.insert(parts, string.format("one of: %s", table.concat(schema.one_of, ", ")))
    end
    if schema.file_exists then
        table.insert(parts, "(file must exist)")
    end
    if schema.dir_exists then
        table.insert(parts, "(directory must exist)")
    end
    return table.concat(parts, " ")
end

describers.number = function(schema)
    local parts = { "number" }
    if schema.min and schema.max then
        local min_op = schema.exclusive_min and ">" or ">="
        local max_op = schema.exclusive_max and "<" or "<="
        table.insert(parts, string.format("(%s %s, %s %s)", min_op, schema.min, max_op, schema.max))
    elseif schema.min then
        local op = schema.exclusive_min and ">" or ">="
        table.insert(parts, string.format("(%s %s)", op, schema.min))
    elseif schema.max then
        local op = schema.exclusive_max and "<" or "<="
        table.insert(parts, string.format("(%s %s)", op, schema.max))
    end
    return table.concat(parts, " ")
end

describers.integer = function(schema)
    local desc = describers.number(schema)
    return desc:gsub("^number", "integer")
end

describers.boolean = function(schema)
    return "boolean"
end

describers["nil"] = function(schema)
    return "nil"
end

describers.any = function(schema)
    return "any"
end

describers.struct = function(schema)
    local fields = {}
    for k, v in pairs(schema.shape) do
        local field_desc = M.describe(v)
        local req = v.required and " (required)" or ""
        table.insert(fields, string.format("  %s: %s%s", k, field_desc, req))
    end
    table.sort(fields)
    if #fields == 0 then
        return "struct {}"
    end
    return "struct {\n" .. table.concat(fields, "\n") .. "\n}"
end

describers.array = function(schema)
    return "array of " .. M.describe(schema.item)
end

describers.tuple = function(schema)
    local items = {}
    for i, item_schema in ipairs(schema.shape) do
        table.insert(items, M.describe(item_schema))
    end
    return "tuple(" .. table.concat(items, ", ") .. ")"
end

describers.dictionary = function(schema)
    return "dict<" .. M.describe(schema.key) .. ", " .. M.describe(schema.value) .. ">"
end

describers.optional = function(schema)
    return M.describe(schema.inner) .. "?"
end

describers.any_of = function(schema)
    local types = {}
    for _, t in ipairs(schema.types) do
        table.insert(types, M.describe(t))
    end
    return table.concat(types, " | ")
end

describers.all_of = function(schema)
    local types = {}
    for _, t in ipairs(schema.types) do
        table.insert(types, M.describe(t))
    end
    return table.concat(types, " & ")
end

describers.literal = function(schema)
    if type(schema.value) == "string" then
        return string.format('"%s"', schema.value)
    end
    return tostring(schema.value)
end

return M
