pub fn early_return(x: i32) -> i32 {
    if x < 0 {
        return -1;
    }
    if x == 0 {
        return 0;
    }
    x * 2
}
