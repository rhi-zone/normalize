pub fn match_(x: i32) -> &'static str {
    match x {
        0 => "zero",
        1 => "one",
        2..=9 => "small",
        _ => "large",
    }
}
