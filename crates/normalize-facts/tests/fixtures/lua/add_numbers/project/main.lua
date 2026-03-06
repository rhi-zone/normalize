local math_utils = require("math_utils")

local function main()
    local sum = math_utils.add(2, 3)
    local product = math_utils.multiply(4, 5)
    print("Sum: " .. sum)
    print("Product: " .. product)
end

main()
