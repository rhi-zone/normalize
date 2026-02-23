fn foo(a: Option<i32>) {
    if let Some(x) = a {
        if let Some(y) = Some(x) {
            println!("{}", y);
        }
    }
}
