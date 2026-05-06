// Fixture for rust/unwrap-in-impl.
// The rule fires on .unwrap() calls inside `impl` blocks.

struct Cache {
    data: std::collections::HashMap<String, String>,
}

impl Cache {
    fn get(&self, key: &str) -> String {
        // This unwrap is flagged — use ? or unwrap_or instead.
        self.data.get(key).unwrap().clone()
    }
}

// Outside an impl block — no finding expected.
fn standalone() -> i32 {
    let x: Option<i32> = Some(42);
    x.unwrap()
}
