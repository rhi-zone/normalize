const std = @import("std");

pub fn add(a: i32, b: i32) i32 {
    return a + b;
}

pub fn multiply(a: i32, b: i32) i32 {
    return a * b;
}

pub const Calculator = struct {
    result: i32,

    pub fn init() Calculator {
        return Calculator{ .result = 0 };
    }

    pub fn compute(self: *Calculator, op: []const u8, a: i32, b: i32) i32 {
        if (std.mem.eql(u8, op, "add")) {
            self.result = add(a, b);
        } else {
            self.result = multiply(a, b);
        }
        return self.result;
    }

    pub fn lastResult(self: *const Calculator) i32 {
        return self.result;
    }
};
