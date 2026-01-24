//! Registry for code generation backends.

use crate::traits::{Backend, BackendCategory};
use std::sync::{OnceLock, RwLock};

/// Global registry of backends.
static BACKENDS: RwLock<Vec<&'static dyn Backend>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Register a custom backend.
///
/// Call this before any generation operations to add custom backends.
/// Built-in backends are registered automatically on first use.
pub fn register_backend(backend: &'static dyn Backend) {
    BACKENDS.write().unwrap().push(backend);
}

/// Initialize built-in backends (called automatically on first use).
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        let mut backends = BACKENDS.write().unwrap();

        #[cfg(feature = "backend-typescript")]
        {
            backends.push(&crate::output::typescript::TYPESCRIPT_BACKEND);
        }

        #[cfg(feature = "backend-zod")]
        {
            backends.push(&crate::output::zod::ZOD_BACKEND);
        }

        #[cfg(feature = "backend-valibot")]
        {
            backends.push(&crate::output::valibot::VALIBOT_BACKEND);
        }

        #[cfg(feature = "backend-python")]
        {
            backends.push(&crate::output::python::PYTHON_BACKEND);
        }

        #[cfg(feature = "backend-pydantic")]
        {
            backends.push(&crate::output::pydantic::PYDANTIC_BACKEND);
        }

        #[cfg(feature = "backend-go")]
        {
            backends.push(&crate::output::go::GO_BACKEND);
        }

        #[cfg(feature = "backend-rust")]
        {
            backends.push(&crate::output::rust::RUST_BACKEND);
        }
    });
}

/// Get a backend by name.
pub fn get_backend(name: &str) -> Option<&'static dyn Backend> {
    init_builtin();
    BACKENDS
        .read()
        .unwrap()
        .iter()
        .find(|b| b.name() == name)
        .copied()
}

/// Get all backends for a language.
pub fn backends_for_language(language: &str) -> Vec<&'static dyn Backend> {
    init_builtin();
    BACKENDS
        .read()
        .unwrap()
        .iter()
        .filter(|b| b.language() == language)
        .copied()
        .collect()
}

/// Get all backends in a category.
pub fn backends_by_category(category: BackendCategory) -> Vec<&'static dyn Backend> {
    init_builtin();
    BACKENDS
        .read()
        .unwrap()
        .iter()
        .filter(|b| b.category() == category)
        .copied()
        .collect()
}

/// List all registered backends.
pub fn backends() -> Vec<&'static dyn Backend> {
    init_builtin();
    BACKENDS.read().unwrap().clone()
}

/// List all registered backend names.
pub fn backend_names() -> Vec<&'static str> {
    init_builtin();
    BACKENDS.read().unwrap().iter().map(|b| b.name()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_lookup() {
        // Ensure we can list backends without panic
        let names = backend_names();
        // At minimum, with default features we should have some backends
        assert!(!names.is_empty() || cfg!(not(feature = "default")));
    }
}
