struct Pair {
    name: String,
    len: usize,
}

fn get_pair() -> Pair {
    Pair { name: String::new(), len: 0 }
}
