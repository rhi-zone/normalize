fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

fn main() {
    let result = add(1, 2);
    println!("{}", greet("world"));
    println!("result = {result}");
}
