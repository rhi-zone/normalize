pub fn loop_(mut x: i32) -> i32 {
    while x > 0 {
        x -= 1;
        if x == 5 {
            break;
        }
    }
    x
}
