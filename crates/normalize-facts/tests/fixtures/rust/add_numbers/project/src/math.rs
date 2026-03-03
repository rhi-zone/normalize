/// Returns the sum of `a` and `b`.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Returns the product of `a` and `b`.
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

/// Stateful calculator that records history.
pub struct Calculator {
    history: Vec<i32>,
}

impl Calculator {
    pub fn new() -> Self {
        Self { history: Vec::new() }
    }

    pub fn compute(&mut self, op: &str, a: i32, b: i32) -> i32 {
        let result = if op == "add" { add(a, b) } else { multiply(a, b) };
        self.history.push(result);
        result
    }
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}
