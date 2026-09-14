//! The Rerun selector runtime, carrying all built-in functions.

use std::sync::{Arc, OnceLock};

use re_lenses_core::Runtime;
use re_lenses_core::function_registry::FunctionRegistry;

/// The default runtime, with all of the built-in functions registered.
///
/// This is the runtime that should back selector and lens execution.
pub fn default_runtime() -> Arc<Runtime> {
    static DEFAULT_RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

    DEFAULT_RUNTIME
        .get_or_init(|| {
            let mut registry = FunctionRegistry::new();
            // TODO(grtlr): This is just an example; `string_prefix` itself should probably be
            // replaced by the corresponding jq operation, and we should be mindful of what we
            // expose in the runtime.
            registry
                .register("string_prefix", |prefix: String| {
                    crate::op::string_prefix(prefix)
                })
                .expect("built-in function names must be unique");
            Arc::new(Runtime::new(Arc::new(registry)))
        })
        .clone()
}
