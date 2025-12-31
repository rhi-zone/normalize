-- @{name}: Description of what this script does
-- Usage: moss @{name} [args...]

-- Help text
local function print_help()
    print([[moss @{name} - Description

Usage: moss @{name} [command] [args...]

Commands:
  (none)    Default action
  help      Show this help

Examples:
  moss @{name}              # run default action
  moss @{name} help         # show help]])
end

-- Main
local action = args[1]

if action == "--help" or action == "-h" or action == "help" then
    print_help()
    os.exit(0)
elseif not action then
    -- Default action
    print("Hello from @{name}!")
    print("Edit .moss/scripts/{name}.lua to customize this script.")
else
    print("Unknown action: " .. action .. "\n")
    print_help()
    os.exit(1)
end
