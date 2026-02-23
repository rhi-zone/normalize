fn foo(a: Option<i32>, b: Option<i32>) {
    if let Some(x) = a {
        println!("{}", x);
    }
    if let (Some(x), Some(y)) = (a, b) {
        println!("{} {}", x, y);
    }
}
