local json = require("json")
local utils = require("utils.string")

-- Simple stack implementation
local Stack = {}
Stack.__index = Stack

function Stack.new()
    return setmetatable({ items = {}, size = 0 }, Stack)
end

function Stack:push(value)
    self.size = self.size + 1
    self.items[self.size] = value
end

function Stack:pop()
    if self.size == 0 then
        return nil
    end
    local value = self.items[self.size]
    self.items[self.size] = nil
    self.size = self.size - 1
    return value
end

function Stack:is_empty()
    return self.size == 0
end

-- Classify a number
function classify(n)
    if n < 0 then
        return "negative"
    elseif n == 0 then
        return "zero"
    else
        return "positive"
    end
end

-- Sum even numbers in a table
function sum_evens(tbl)
    local total = 0
    for _, v in ipairs(tbl) do
        if v % 2 == 0 then
            total = total + v
        end
    end
    return total
end

-- Count occurrences of each element
function count_occurrences(tbl)
    local counts = {}
    for _, v in ipairs(tbl) do
        counts[v] = (counts[v] or 0) + 1
    end
    return counts
end

local s = Stack.new()
s:push(1)
s:push(2)
print(classify(-3))
print(sum_evens({1, 2, 3, 4, 5}))
