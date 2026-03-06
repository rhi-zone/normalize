fn simple(a: Option<i32>) {
    if let Some(x) = a && let Some(y) = Some(x) {
            println!("{}", y);
        }
}
