//! sqlite-vec extension registration.
//!
//! sqlite-vec provides a `vec0` virtual table for approximate nearest-neighbor
//! search. The crate (`sqlite-vec`) ships a static C library that must be
//! registered with SQLite on each connection that wants `vec0` tables.
//!
//! # Per-connection registration
//!
//! We call `sqlite3_vec_init(db, NULL, NULL)` directly on the connection's
//! underlying raw `*mut sqlite3` handle.  This avoids `sqlite3_auto_extension`,
//! which internally calls `sqlite3_initialize()` and conflicts with libsql's
//! own initialization (libsql asserts that `sqlite3_config(SERIALIZED)` returns
//! `SQLITE_OK`, which fails if sqlite was already initialized).
//!
//! Because `libsql::Connection` does not expose the raw handle through its
//! public API, we obtain it by opening a lightweight raw FFI connection to the
//! same database file.  A `VecConnection` wraps this raw handle and provides
//! helper methods for vec-specific operations.
//!
//! # Safety
//!
//! `sqlite3_vec_init` is declared by the `sqlite-vec` crate as `fn()` (zero
//! arguments) but the actual C function has the standard SQLite extension entry
//! point signature `(sqlite3*, char**, const sqlite3_api_routines*) -> int`.
//! We re-declare it with the correct signature and call it directly.  Because
//! the library is compiled with `SQLITE_CORE`, the `pApi` parameter is ignored
//! (the extension calls SQLite functions directly rather than through the API
//! struct).

use libsql::ffi;
use std::ffi::CString;
use std::path::Path;

/// The correct C signature for `sqlite3_vec_init`.  The `sqlite-vec` crate
/// declares it as `fn()` (zero arguments), but the real symbol is a standard
/// SQLite extension entry point: `(sqlite3*, char**, const sqlite3_api_routines*) -> int`.
type VecInitFn = unsafe extern "C" fn(
    *mut ffi::sqlite3,
    *mut *mut ::std::os::raw::c_char,
    *const ffi::sqlite3_api_routines,
) -> ::std::os::raw::c_int;

/// Register the sqlite-vec extension on a specific raw SQLite handle.
///
/// # Safety
///
/// `db` must be a valid, open `sqlite3*` handle.
unsafe fn register_vec_on_handle(db: *mut ffi::sqlite3) -> bool {
    // Transmute `sqlite_vec::sqlite3_vec_init` (declared as `fn()`) to the
    // correct 3-argument entry-point signature.  This is safe because the
    // underlying C function actually has that signature — the crate just
    // declares it with zero args for use with `sqlite3_auto_extension`.
    let init_fn: VecInitFn =
        unsafe { std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ()) };
    let rc = unsafe { init_fn(db, std::ptr::null_mut(), std::ptr::null()) };
    rc == ffi::SQLITE_OK
}

/// A raw SQLite connection with sqlite-vec registered.
///
/// This wraps a `*mut sqlite3` handle opened via FFI and provides vec-specific
/// operations.  The handle points to the same database file as the main
/// `libsql::Connection`, allowing vec operations (ANN virtual table) alongside
/// normal queries on the main connection.
pub struct VecConnection {
    raw: *mut ffi::sqlite3,
}

// SAFETY: The underlying sqlite3 handle is compiled with SQLITE_THREADSAFE=1
// (serialized mode), so it's safe to send/share across threads.
unsafe impl Send for VecConnection {}
unsafe impl Sync for VecConnection {}

impl VecConnection {
    /// Open a raw connection to `db_path` and register sqlite-vec on it.
    ///
    /// Returns `None` if the path can't be converted to a C string or if
    /// opening / extension registration fails.
    pub fn open(db_path: &Path) -> Option<Self> {
        let c_path = CString::new(db_path.to_str()?).ok()?;
        let mut raw: *mut ffi::sqlite3 = std::ptr::null_mut();
        let rc = unsafe {
            ffi::sqlite3_open_v2(
                c_path.as_ptr(),
                &mut raw,
                ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE,
                std::ptr::null(),
            )
        };
        if rc != ffi::SQLITE_OK || raw.is_null() {
            if !raw.is_null() {
                unsafe { ffi::sqlite3_close(raw) };
            }
            return None;
        }

        if !unsafe { register_vec_on_handle(raw) } {
            unsafe { ffi::sqlite3_close(raw) };
            return None;
        }

        Some(VecConnection { raw })
    }

    /// Execute a SQL statement with no parameters.
    pub fn execute(&self, sql: &str) -> Result<(), String> {
        let c_sql = CString::new(sql).map_err(|e| e.to_string())?;
        let rc = unsafe {
            ffi::sqlite3_exec(
                self.raw,
                c_sql.as_ptr(),
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc == ffi::SQLITE_OK {
            Ok(())
        } else {
            Err(format!("sqlite3_exec failed with code {rc}"))
        }
    }

    /// Prepare a SQL statement and return a raw handle for manual binding.
    ///
    /// The caller is responsible for stepping, reading results, and
    /// finalizing via [`VecStmt`].
    pub fn prepare(&self, sql: &str) -> Result<VecStmt, String> {
        let c_sql = CString::new(sql).map_err(|e| e.to_string())?;
        let mut stmt: *mut ffi::sqlite3_stmt = std::ptr::null_mut();
        let rc = unsafe {
            ffi::sqlite3_prepare_v2(
                self.raw,
                c_sql.as_ptr(),
                -1,
                &mut stmt,
                std::ptr::null_mut(),
            )
        };
        if rc != ffi::SQLITE_OK {
            return Err(format!("prepare failed: {rc}"));
        }
        Ok(VecStmt { raw: stmt })
    }

    /// Get the raw sqlite3 handle (for direct FFI use in store operations).
    pub fn handle(&self) -> *mut ffi::sqlite3 {
        self.raw
    }

    /// Return the rowid returned by `last_insert_rowid()`.
    pub fn last_insert_rowid(&self) -> i64 {
        unsafe { ffi::sqlite3_last_insert_rowid(self.raw) }
    }
}

impl Drop for VecConnection {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { ffi::sqlite3_close(self.raw) };
        }
    }
}

/// A prepared statement on a [`VecConnection`].
///
/// Provides methods for binding parameters, stepping, and reading results
/// via raw SQLite FFI.
pub struct VecStmt {
    raw: *mut ffi::sqlite3_stmt,
}

impl VecStmt {
    /// Bind an integer at 1-based position `idx`.
    pub fn bind_int64(&self, idx: i32, val: i64) {
        unsafe { ffi::sqlite3_bind_int64(self.raw, idx, val) };
    }

    /// Bind a BLOB at 1-based position `idx`.
    pub fn bind_blob(&self, idx: i32, data: &[u8]) {
        unsafe {
            ffi::sqlite3_bind_blob(
                self.raw,
                idx,
                data.as_ptr() as *const _,
                data.len() as i32,
                ffi::SQLITE_TRANSIENT(),
            );
        }
    }

    /// Bind a text string at 1-based position `idx`.
    pub fn bind_text(&self, idx: i32, val: &str) {
        let c_val = CString::new(val).unwrap_or_default();
        unsafe {
            ffi::sqlite3_bind_text(self.raw, idx, c_val.as_ptr(), -1, ffi::SQLITE_TRANSIENT());
        }
    }

    /// Step the statement.  Returns `true` if a row is available
    /// (`SQLITE_ROW`), `false` on `SQLITE_DONE`.  Other codes return an error.
    pub fn step(&self) -> Result<bool, String> {
        let rc = unsafe { ffi::sqlite3_step(self.raw) };
        match rc {
            _ if rc == ffi::SQLITE_ROW => Ok(true),
            _ if rc == ffi::SQLITE_DONE => Ok(false),
            _ => Err(format!("step failed: {rc}")),
        }
    }

    /// Read a 64-bit integer from column `idx` (0-based).
    pub fn column_int64(&self, idx: i32) -> i64 {
        unsafe { ffi::sqlite3_column_int64(self.raw, idx) }
    }

    /// Read a double from column `idx` (0-based).
    pub fn column_double(&self, idx: i32) -> f64 {
        unsafe { ffi::sqlite3_column_double(self.raw, idx) }
    }

    /// Read a text string from column `idx` (0-based).
    pub fn column_text(&self, idx: i32) -> Option<String> {
        let ptr = unsafe { ffi::sqlite3_column_text(self.raw, idx) };
        if ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(ptr as *const _) };
            c_str.to_str().ok().map(|s| s.to_string())
        }
    }

    /// Read a BLOB from column `idx` (0-based).
    pub fn column_blob(&self, idx: i32) -> Vec<u8> {
        let ptr = unsafe { ffi::sqlite3_column_blob(self.raw, idx) };
        let len = unsafe { ffi::sqlite3_column_bytes(self.raw, idx) };
        if ptr.is_null() || len <= 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) }.to_vec()
        }
    }
}

impl Drop for VecStmt {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { ffi::sqlite3_finalize(self.raw) };
        }
    }
}

/// Register the sqlite-vec extension on a `libsql::Connection` by opening a
/// raw FFI handle to the same database and registering there.
///
/// This is a convenience for the common case where you have a
/// `libsql::Connection` and a `db_path` and want vec on a parallel handle.
/// Returns the `VecConnection` if successful.
pub fn open_vec_connection(db_path: &Path) -> Option<VecConnection> {
    VecConnection::open(db_path)
}

/// Check whether the sqlite-vec extension is available on a connection by
/// trying to call `vec_version()`.  Returns `true` if the extension is loaded.
pub async fn vec_available(conn: &libsql::Connection) -> bool {
    match conn.query("SELECT vec_version()", ()).await {
        Ok(mut rows) => rows.next().await.is_ok(),
        Err(_) => false,
    }
}
