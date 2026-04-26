use std::collections::HashMap;
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Counter {
    counts: HashMap<String, usize>,
}

impl Counter {
    pub fn new() -> Self {
        Counter {
            counts: HashMap::new(),
        }
    }

    pub fn increment(&mut self, key: &str) {
        let entry = self.counts.entry(key.to_string()).or_insert(0);
        *entry += 1;
    }

    pub fn get(&self, key: &str) -> usize {
        *self.counts.get(key).unwrap_or(&0)
    }
}

impl Display for Counter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, v) in &self.counts {
            writeln!(f, "{k}: {v}")?;
        }
        Ok(())
    }
}

/// Classify a number
pub fn classify(n: i32) -> &'static str {
    if n < 0 {
        "negative"
    } else if n == 0 {
        "zero"
    } else {
        "positive"
    }
}

pub fn sum_evens(values: &[i32]) -> i32 {
    let mut total = 0;
    for v in values {
        if v % 2 == 0 {
            total += v;
        }
    }
    total
}
