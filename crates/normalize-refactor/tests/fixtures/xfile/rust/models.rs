pub struct Calculator {
    pub value: i32,
}

impl Calculator {
    pub fn new(initial: i32) -> Self {
        Calculator { value: initial }
    }
}
