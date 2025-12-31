-- @{name}: Description of what this script does
-- Usage: moss @{name} [command] [args...]

-- Help text
local function print_help()
    print([[moss @{name} - Description

Usage: moss @{name} [command] [args...]

Commands:
  list          List items
  add <text>    Add a new item
  rm <query>    Remove an item

Examples:
  moss @{name}              # list items (default)
  moss @{name} add foo      # add item
  moss @{name} rm foo       # remove item]])
end

-- Commands
local function cmd_list()
    print("TODO: implement list")
end

local function cmd_add(text)
    if not text or text == "" then
        print("Usage: moss @{name} add <text>")
        os.exit(1)
    end
    print("TODO: implement add: " .. text)
end

local function cmd_rm(query)
    if not query or query == "" then
        print("Usage: moss @{name} rm <query>")
        os.exit(1)
    end
    print("TODO: implement rm: " .. query)
end

-- Main
local action = args[1]

if action == "--help" or action == "-h" or action == "help" then
    print_help()
    os.exit(0)
elseif not action or action == "list" then
    cmd_list()
elseif action == "add" then
    cmd_add(table.concat(args, " ", 2))
elseif action == "rm" then
    cmd_rm(table.concat(args, " ", 2))
else
    print("Unknown action: " .. action .. "\n")
    print_help()
    os.exit(1)
end
