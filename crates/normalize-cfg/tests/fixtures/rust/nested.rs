pub fn nested(mut x: i32) -> i32 {
    let mut sum = 0;
    for i in 0..x {
        if i % 2 == 0 {
            sum += i;
        } else {
            sum -= 1;
        }
    }
    sum
}
