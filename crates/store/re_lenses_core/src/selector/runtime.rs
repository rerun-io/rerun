//! Shared runtime context for selector evaluation.

use std::sync::{Arc, OnceLock};

use super::function_registry::FunctionRegistry;

pub(super) fn default_runtime() -> Arc<Runtime> {
    static DEFAULT_RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

    DEFAULT_RUNTIME
        .get_or_init(|| {
            Arc::new(Runtime {
                function_registry: Arc::new(FunctionRegistry::default()),
            })
        })
        .clone()
}

/// Context passed to selector execution.
///
/// Carries the [`FunctionRegistry`] and any future shared state
/// needed during evaluation.
#[derive(Clone)]
pub struct Runtime {
    pub function_registry: Arc<FunctionRegistry>,
}

impl re_byte_size::SizeBytes for Runtime {
    fn heap_size_bytes(&self) -> u64 {
        let Self { function_registry } = self;

        function_registry.heap_size_bytes()
    }
}
