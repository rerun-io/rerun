//! Runtime execution of [`Expr`] against Arrow [`ListArray`]s.
//!
//! This module implements execution of expressions against Arrow [`ListArray`]s.

use std::sync::{Arc, OnceLock};

use arrow::array::{Array as _, FixedSizeListArray, ListArray};

use crate::combinators::{
    Flatten, GetField, GetIndexList, MapList, PromoteInnerNulls, Transform as _,
};

use super::function_registry::FunctionRegistry;

pub fn default_runtime() -> Arc<Runtime> {
    static DEFAULT_RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

    DEFAULT_RUNTIME
        .get_or_init(|| {
            Arc::new(Runtime {
                function_registry: FunctionRegistry::default(),
            })
        })
        .clone()
}

/// Context passed to selector execution.
///
/// Carries the [`FunctionRegistry`] and any future shared state
/// needed during evaluation.
pub struct Runtime {
    pub function_registry: FunctionRegistry,
}

impl re_byte_size::SizeBytes for Runtime {
    fn heap_size_bytes(&self) -> u64 {
        let Self { function_registry } = self;

        function_registry.heap_size_bytes()
    }
}

use super::parser::{Expr, Segment, SegmentKind};

/// Executes the given expression against the source array.
///
/// Returns `None` if the expression was suppressed (e.g. `.field?`).
/// The caller decides how to handle the absent result.
pub fn execute_per_row(
    expr: &Expr,
    source: &ListArray,
    runtime: &Runtime,
) -> Result<Option<ListArray>, crate::combinators::Error> {
    // TODO(grtlr): It would be much cleaner if `MapList` (or equivalent would be called on this level).
    let result = expr.execute(source, runtime)?;

    if let Some(ref result) = result {
        re_log::debug_assert_eq!(
            result.len(),
            source.len(),
            "selectors should never change row count"
        );
    }

    Ok(result)
}

fn values_downcasts_to<T: 'static>(array: &ListArray) -> bool {
    array.values().as_any().downcast_ref::<T>().is_some()
}

impl SegmentKind {
    fn execute(&self, source: &ListArray) -> Result<Option<ListArray>, crate::combinators::Error> {
        match self {
            Self::Field(field_name) => {
                MapList::new(GetField::new(field_name.clone())).transform(source)
            }
            Self::Index(index) => MapList::new(GetIndexList::new(*index)).transform(source),
            Self::Each => {
                // In Arrow's columnar context, [] flattens one level of list nesting
                // while preserving row count, rather than exploding to create new rows.
                // This reinterprets jq's streaming iteration as structural unwrapping.
                if values_downcasts_to::<ListArray>(source)
                    || values_downcasts_to::<FixedSizeListArray>(source)
                {
                    // Flatten nested lists: List<List<T>> -> List<T>
                    Flatten::new().transform(source)
                } else {
                    Err(crate::combinators::Error::TypeMismatch {
                        expected: "ListArray".into(),
                        actual: source.value_type(),
                        context: "Each ([]) operator requires nested lists".into(),
                    })
                }
            }
        }
    }
}

impl Segment {
    fn execute(&self, source: &ListArray) -> Result<Option<ListArray>, crate::combinators::Error> {
        let result = match self.kind.execute(source) {
            Ok(result) => result,
            // TODO(RR-3435): FixedSizeListArray errors must be suppressed via `?`, but ListArray should not need it.
            Err(err) if self.suppressed => {
                re_log::trace!("Suppressed segment `{self}` suppressed error: {err}");
                return Ok(None);
            }
            Err(err) => return Err(err),
        };

        let Some(result) = result else {
            return Ok(None);
        };

        if self.assert_non_null {
            PromoteInnerNulls.transform(&result)
        } else {
            Ok(Some(result))
        }
    }
}

impl Expr {
    fn execute(
        &self,
        source: &ListArray,
        runtime: &Runtime,
    ) -> Result<Option<ListArray>, crate::combinators::Error> {
        match self {
            Self::Identity => Ok(Some(source.clone())),
            Self::Path(segments) => {
                let mut result = source.clone();
                for segment in segments {
                    match segment.execute(&result)? {
                        Some(next) => result = next,
                        None => return Ok(None),
                    }
                }
                Ok(Some(result))
            }
            Self::Pipe(left, right) => match left.as_ref().execute(source, runtime)? {
                Some(intermediate) => right.as_ref().execute(&intermediate, runtime),
                None => Ok(None),
            },
            Self::Function { name, arguments } => {
                let function = runtime
                    .function_registry
                    .get(name, arguments.as_ref().map_or(&[], |v| v.as_slice()))?;
                function.transform(source)
            }
        }
    }
}
