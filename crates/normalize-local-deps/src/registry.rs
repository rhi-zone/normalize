//! Local deps registry with ecosystem-key-based lookup.

use crate::LocalDeps;
use std::sync::{OnceLock, RwLock};

/// Global local deps registry.
static DEPS: RwLock<Vec<&'static dyn LocalDeps>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Register a local deps implementation in the global registry.
pub fn register(deps: &'static dyn LocalDeps) {
    DEPS.write().unwrap().push(deps);
}

/// Initialize built-in local deps implementations (called once).
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        #[cfg(feature = "lang-python")]
        register(&crate::python::PythonDeps);
        #[cfg(feature = "lang-rust")]
        register(&crate::rust_lang::RustDeps);
        #[cfg(feature = "lang-javascript")]
        register(&crate::javascript::JavaScriptDeps);
        #[cfg(feature = "lang-typescript")]
        {
            register(&crate::typescript::TypeScriptDeps);
            register(&crate::typescript::TsxDeps);
        }
        #[cfg(feature = "lang-go")]
        register(&crate::go::GoDeps);
        #[cfg(feature = "lang-java")]
        register(&crate::java::JavaDeps);
        #[cfg(feature = "lang-kotlin")]
        register(&crate::kotlin::KotlinDeps);
        #[cfg(feature = "lang-c")]
        register(&crate::c::CDeps);
        #[cfg(feature = "lang-cpp")]
        register(&crate::cpp::CppDeps);
    });
}

/// Get all registered local deps implementations.
pub fn all_local_deps() -> Vec<&'static dyn LocalDeps> {
    init_builtin();
    DEPS.read().unwrap().clone()
}

/// Find a local deps implementation by ecosystem key.
pub fn deps_for_ecosystem(key: &str) -> Option<&'static dyn LocalDeps> {
    init_builtin();
    let deps = DEPS.read().unwrap();
    deps.iter().find(|d| d.ecosystem_key() == key).copied()
}

/// Find a local deps implementation by language name.
pub fn deps_for_language(name: &str) -> Option<&'static dyn LocalDeps> {
    init_builtin();
    let deps = DEPS.read().unwrap();
    deps.iter().find(|d| d.language_name() == name).copied()
}
