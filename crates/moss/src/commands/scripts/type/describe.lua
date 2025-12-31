-- Human-readable schema descriptions
-- Usage: local describe = require("type.describe")

local describers = {}

--- Generate a human-readable description of a schema
--- @param schema table
--- @return string
local function describe(schema)
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

describers.boolean = function()
    return "boolean"
end

describers["nil"] = function()
    return "nil"
end

describers.any = function()
    return "any"
end

describers.struct = function(schema)
    local fields = {}
    for k, v in pairs(schema.shape) do
        local field_desc = describe(v)
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
    return "array of " .. describe(schema.item)
end

describers.tuple = function(schema)
    local items = {}
    for _, item_schema in ipairs(schema.shape) do
        table.insert(items, describe(item_schema))
    end
    return "tuple(" .. table.concat(items, ", ") .. ")"
end

describers.dictionary = function(schema)
    return "dict<" .. describe(schema.key) .. ", " .. describe(schema.value) .. ">"
end

describers.optional = function(schema)
    return describe(schema.inner) .. "?"
end

describers.any_of = function(schema)
    local types = {}
    for _, t in ipairs(schema.types) do
        table.insert(types, describe(t))
    end
    return table.concat(types, " | ")
end

describers.all_of = function(schema)
    local types = {}
    for _, t in ipairs(schema.types) do
        table.insert(types, describe(t))
    end
    return table.concat(types, " & ")
end

describers.literal = function(schema)
    if type(schema.value) == "string" then
        return string.format('"%s"', schema.value)
    end
    return tostring(schema.value)
end

return describe
