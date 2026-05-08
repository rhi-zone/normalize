mod utils;
mod models;

use models::Calculator;

fn main() {
    let calc = Calculator::new(utils::add(1, 2));
    println!("{}", calc.value);
}
