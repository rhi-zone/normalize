//! Runtime utilities for running async code from sync contexts.

/// Run an async future to completion, working whether or not a tokio runtime is active.
///
/// - Inside a tokio runtime: uses `block_in_place` to avoid runtime nesting panics.
/// - Outside a runtime: creates a new single-thread runtime.
pub fn block_on<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(fut),
    }
}

/// Run an async future to completion, returning Err on tokio runtime creation failure.
/// Same as `block_on` but for functions that return Result.
pub fn try_block_on<F, T>(fut: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    Ok(block_on(fut))
}
