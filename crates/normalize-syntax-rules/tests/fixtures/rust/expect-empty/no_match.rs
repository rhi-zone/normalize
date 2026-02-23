fn foo() {
    let x: Option<i32> = Some(1);
    let _ = x.expect("x should be set at this point");
}
