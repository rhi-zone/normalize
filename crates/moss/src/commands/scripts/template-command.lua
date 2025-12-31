-- @{name}: Description of what this script does
local cli = require("cli")

cli.run {
    name = "{name}",
    description = "Description of what this script does",

    commands = {
        { name = "list", description = "List items", default = true,
          run = function(args)
              print("TODO: implement list")
          end },

        { name = "add", description = "Add a new item",
          args = { "text..." },
          run = function(args)
              print("TODO: implement add: " .. (args.text or ""))
          end },

        { name = "rm", description = "Remove an item",
          args = { "query" },
          run = function(args)
              print("TODO: implement rm: " .. (args.query or ""))
          end },
    },
}
