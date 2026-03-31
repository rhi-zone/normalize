//! sqlite-vec extension registration.
//!
//! sqlite-vec provides a `vec0` virtual table for approximate nearest-neighbor
//! search. The crate (`sqlite-vec`) ships a static C library that must be
//! registered with SQLite before any connection that wants the `vec0` table is
//! opened.
//!
//! SQLite's `sqlite3_auto_extension` registers an init function that is called
//! automatically on every new connection — effectively making the extension
//! globally available for the process lifetime once installed.
//!
//! # Safety
//!
//! `register_vec_extension` calls `sqlite3_auto_extension` via raw FFI, which
//! is inherently unsafe. The transmute is required because sqlite-vec's
//! `sqlite3_vec_init` uses the no-argument entry-point convention, while
//! `sqlite3_auto_extension` expects the 3-argument form. SQLite calls the
//! registered function with the full `(db, pzErrMsg, pApi)` signature, but the
//! entry point may ignore trailing arguments — this is how all loadable SQLite
//! extensions work. The transmute is therefore sound for this use case.
//!
//! Call [`register_vec_extension`] exactly once, before any `libsql` connection
//! is opened (e.g. at process start or via `std::sync::OnceLock`).

use std::sync::OnceLock;

static VEC_REGISTERED: OnceLock<bool> = OnceLock::new();

/// Register the sqlite-vec extension with SQLite's auto-extension mechanism.
///
/// Idempotent — calling it multiple times is safe (guarded by `OnceLock`).
/// Returns `true` on the first call that actually registers, `false` on
/// subsequent calls.
pub fn register_vec_extension() -> bool {
    *VEC_REGISTERED.get_or_init(|| {
        // SAFETY: We transmute `sqlite3_vec_init` (zero-argument entry point
        // from the sqlite-vec static library) to the 3-argument form that
        // `sqlite3_auto_extension` expects.  The C calling convention on all
        // supported platforms passes arguments left-to-right in registers; a
        // callee that takes no arguments simply ignores any extras.  This is the
        // canonical pattern for registering embedded SQLite extensions.
        unsafe {
            libsql::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                unsafe extern "C" fn(),
                unsafe extern "C" fn(
                    *mut libsql::ffi::sqlite3,
                    *mut *const ::std::os::raw::c_char,
                    *const libsql::ffi::sqlite3_api_routines,
                ) -> ::std::os::raw::c_int,
            >(sqlite_vec::sqlite3_vec_init)));
        }
        true
    })
}

/// Check whether the sqlite-vec extension is available on a connection by
/// trying to call `vec_version()`.  Returns `true` if the extension is loaded.
pub async fn vec_available(conn: &libsql::Connection) -> bool {
    match conn.query("SELECT vec_version()", ()).await {
        Ok(mut rows) => rows.next().await.is_ok(),
        Err(_) => false,
    }
}
