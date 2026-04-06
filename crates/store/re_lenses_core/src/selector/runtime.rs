//! Shared runtime context for selector evaluation.

use std::sync::{Arc, OnceLock};

use arrow::array::{ArrayRef, ListArray};
use re_chunk::ArrowArray as _;

use super::Selector;
use super::eval;
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

impl Runtime {
    /// Execute a selector against a raw array using this runtime.
    ///
    /// This is the `ArrayRef`-based entry point. For per-row execution
    /// on a [`ListArray`], use [`execute_per_row`](Self::execute_per_row).
    pub fn execute<E: eval::Eval>(
        &self,
        selector: &Selector<E>,
        source: ArrayRef,
    ) -> Result<Option<ArrayRef>, super::Error> {
        eval::execute(&selector.expr, source, self).map_err(Into::into)
    }

    /// Execute a selector against each row of a [`ListArray`] using this runtime.
    ///
    /// Performs implicit iteration over the inner list array, and reconstructs the array at the end.
    ///
    /// `map(.poses[].x)` is the actual query, we only require writing the `.poses[].x` portion.
    ///
    /// Returns `None` if the expression's error was suppressed (e.g. `.field?`).
    pub fn execute_per_row<E: eval::Eval>(
        &self,
        selector: &Selector<E>,
        source: &ListArray,
    ) -> Result<Option<ListArray>, super::Error> {
        let res = eval::eval_map(source, &selector.expr, self).map_err(Into::into);

        if let Ok(Some(ref output)) = res {
            re_log::debug_assert_eq!(
                output.len(),
                source.len(),
                "selectors should never change row count"
            );
        }

        res
    }
}

impl re_byte_size::SizeBytes for Runtime {
    fn heap_size_bytes(&self) -> u64 {
        let Self { function_registry } = self;

        function_registry.heap_size_bytes()
    }
}
