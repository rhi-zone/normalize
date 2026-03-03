mod math;

use math::Calculator;

fn main() {
    let mut calc = Calculator::new();
    println!("{}", calc.compute("add", 2, 3));
    println!("{}", calc.compute("mul", 4, 5));
}
