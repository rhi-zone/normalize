type Result<T> = std::result::Result<T, MyError>;
type Callback = Box<dyn Fn(i32) -> i32>;
