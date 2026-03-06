const std = @import("std");
const math_utils = @import("math_utils.zig");

pub fn main() void {
    const sum = math_utils.add(2, 3);
    const product = math_utils.multiply(4, 5);

    const stdout = std.io.getStdOut().writer();
    stdout.print("Sum: {}\n", .{sum}) catch {};
    stdout.print("Product: {}\n", .{product}) catch {};

    var calc = math_utils.Calculator.init();
    _ = calc.compute("add", 10, 20);
    stdout.print("Result: {}\n", .{calc.lastResult()}) catch {};
}
