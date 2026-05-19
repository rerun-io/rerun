//! Evaluation of [`Expr`] and [`DynExpr`] against Arrow arrays.

use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, AsArray as _, BooleanBufferBuilder, ListArray, OffsetSizeTrait,
};
use arrow::buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;

use crate::combinators::{GetField, GetIndexList, Transform as _};

use super::DynExpr;
use super::parser::Expr;
use super::runtime::Runtime;

/// Internal trait for expression types that can be evaluated against Arrow arrays.
pub trait Eval {
    fn eval(
        &self,
        source: ArrayRef,
        runtime: &Runtime,
    ) -> Result<Option<EvalResult>, crate::combinators::Error>;
}

/// Result of evaluating an expression on a flat array.
///
/// When evaluating a selector like `.poses[].x` that contains an `Expr::Each`,
/// the expression runs on the flattened inner values. This means the result is a flat
/// array that needs to be reassembled back into a `ListArray` when collecting it,
/// for example in a `Expr::Map` with the correct row boundaries. [`EvalResult`]
/// carries the bookkeeping needed for that reassembly.
///
/// NOTE: This is essentially a destructured [`ListArray`].
pub struct EvalResult {
    /// The transformed values array.
    array: ArrayRef,

    /// Optional offsets introduced by `Each` (`[]`) operations.
    ///
    /// When present, these map "intermediate groups" to positions in `array`.
    /// They get composed with the outer row offsets in `execute_per_row`.
    ///
    /// When `None`, it means that there is a 1:1 mapping between surrounding rows
    /// and the current `array`.
    offsets: Option<OffsetBuffer<i32>>,

    /// Optional null buffer from `NonNull` (`!`) operations.
    ///
    /// When present, marks which intermediate groups are null.
    /// Has length `offsets.len() - 1` if offsets are present, or `array.len()` otherwise.
    nulls: Option<NullBuffer>,
}

impl EvalResult {
    fn flat(array: ArrayRef) -> Self {
        Self {
            array,
            offsets: None,
            nulls: None,
        }
    }
}

/// Compose two optional offset buffers (outer ∘ inner).
fn compose_offsets<O: OffsetSizeTrait>(
    outer: Option<&OffsetBuffer<O>>,
    inner: Option<&OffsetBuffer<O>>,
) -> Option<OffsetBuffer<O>> {
    match (outer, inner) {
        (None, None) => None,
        (Some(o), None) => Some(o.clone()),
        (None, Some(i)) => Some(i.clone()),
        (Some(a), Some(b)) => Some(compose_offset_buffers(a, b)),
    }
}

/// Given outer offsets mapping N → M and inner offsets mapping M → K,
/// produce composed offsets mapping N → K.
fn compose_offset_buffers<O: OffsetSizeTrait>(
    outer: &OffsetBuffer<O>,
    inner: &OffsetBuffer<O>,
) -> OffsetBuffer<O> {
    let scalars: ScalarBuffer<O> = outer.iter().map(|&o| inner[o.as_usize()]).collect();
    OffsetBuffer::new(scalars)
}

/// Promote inner nulls to outer nulls on an `EvalResult`.
///
/// For each group (defined by `offsets`, or one-element-per-group if `None`),
/// the group is marked null if ALL values within it are null.
fn promote_inner_nulls(result: EvalResult) -> EvalResult {
    let Some(inner_nulls) = result.array.logical_nulls() else {
        // No inner nulls at all → nothing to promote
        return result;
    };

    let promoted = match &result.offsets {
        Some(offsets) => aggregate_nulls(offsets, &inner_nulls),
        None => inner_nulls, // 1:1: each value is its own group
    };

    EvalResult {
        array: result.array,
        offsets: result.offsets,
        nulls: combine_null_buffers(result.nulls.as_ref(), Some(&promoted)),
    }
}

/// Aggregate per-group nulls into per-row nulls.
///
/// For each row (defined by `outer_offsets`), the row is null if ALL
/// intermediate groups in the row are null according to `eval_nulls`.
fn aggregate_nulls(outer_offsets: &OffsetBuffer<i32>, inner_nulls: &NullBuffer) -> NullBuffer {
    let num_rows = outer_offsets.len() - 1;
    let mut buf = BooleanBufferBuilder::new(num_rows);

    for row in 0..num_rows {
        let start = outer_offsets[row] as usize;
        let end = outer_offsets[row + 1] as usize;

        if start == end {
            // Empty row: keep as valid
            buf.append(true);
        } else {
            // Row is valid if ANY group in it is valid
            buf.append((start..end).any(|i| inner_nulls.is_valid(i)));
        }
    }

    NullBuffer::from(buf.finish())
}

/// Combine two optional null buffers with AND.
fn combine_null_buffers(a: Option<&NullBuffer>, b: Option<&NullBuffer>) -> Option<NullBuffer> {
    match (a, b) {
        (None, None) => None,
        (Some(n), None) | (None, Some(n)) => Some(n.clone()),
        (Some(a), Some(b)) => {
            let combined = a.inner() & b.inner();
            Some(NullBuffer::new(combined))
        }
    }
}

/// Executes the given expression against a raw array.
///
/// This is the `ArrayRef`-based entry point used by `Selector::execute`.
pub(super) fn execute<E: Eval>(
    expr: &E,
    source: ArrayRef,
    runtime: &Runtime,
) -> Result<Option<ArrayRef>, crate::combinators::Error> {
    let result = expr.eval(source, runtime)?;
    Ok(result.map(|r| r.array))
}

/// Evaluate an expression within a [`ListArray`].
///
/// Decomposes the list, evaluates the expression on the inner values,
/// and reconstructs a [`ListArray`] by composing offsets and nulls.
pub(super) fn eval_map<E: Eval>(
    list: &ListArray,
    body: &E,
    runtime: &Runtime,
) -> Result<Option<ListArray>, crate::combinators::Error> {
    let (_, outer_offsets, values, outer_nulls) = list.clone().into_parts();

    let Some(result) = body.eval(values, runtime)? else {
        return Ok(None);
    };

    // Compose offsets: outer maps rows → intermediate, eval maps intermediate → values
    let final_offsets = match &result.offsets {
        Some(eval_offsets) => compose_offset_buffers(&outer_offsets, eval_offsets),
        None => outer_offsets.clone(),
    };

    // Combine nulls
    let final_nulls = match &result.nulls {
        Some(eval_nulls) => {
            let row_nulls = aggregate_nulls(&outer_offsets, eval_nulls);
            combine_null_buffers(outer_nulls.as_ref(), Some(&row_nulls))
        }
        None => outer_nulls,
    };

    let new_field = Arc::new(Field::new_list_field(
        result.array.data_type().clone(),
        true,
    ));

    Ok(Some(ListArray::new(
        new_field,
        final_offsets,
        result.array,
        final_nulls,
    )))
}

impl Eval for Expr {
    fn eval(
        &self,
        source: ArrayRef,
        runtime: &Runtime,
    ) -> Result<Option<EvalResult>, crate::combinators::Error> {
        match self {
            Self::Identity => Ok(Some(EvalResult::flat(source))),

            Self::Field(field_name) => match source.data_type() {
                DataType::Struct(..) => {
                    let struct_array = source.as_struct();
                    match GetField::new(field_name.clone()).transform(struct_array)? {
                        Some(field_array) => Ok(Some(EvalResult::flat(field_array))),
                        None => Ok(None),
                    }
                }
                dt => Err(ArrowError::InvalidArgumentError(format!(
                    "cannot access field `.{field_name}` on unexpected type {dt}"
                )))?,
            },

            Self::Index(index) => match source.data_type() {
                DataType::List(_) => {
                    let list_array = source.as_list::<i32>();
                    match GetIndexList::new(*index).transform(list_array)? {
                        Some(result) => Ok(Some(EvalResult::flat(result))),
                        None => Ok(None),
                    }
                }
                // TODO(RR-3435): Add indexing into `FixedSizeListArray`.
                dt @ DataType::FixedSizeList(..) => Err(ArrowError::NotYetImplemented(format!(
                    "index access `[{index}]` is not yet implemented for {dt}"
                )))?,
                dt => Err(ArrowError::InvalidArgumentError(format!(
                    "cannot access `[{index}]` on unexpected type {dt}"
                )))?,
            },

            Self::Each => match source.data_type() {
                DataType::List(_) => {
                    let list_array = source.as_list::<i32>().clone();
                    Ok(Some({
                        let (_, offsets, values, nulls) = list_array.into_parts();
                        EvalResult {
                            array: values,
                            offsets: Some(offsets),
                            nulls,
                        }
                    }))
                }
                DataType::FixedSizeList(_, _) => {
                    let fixed = source.as_fixed_size_list().clone();
                    let len = i32::try_from(fixed.len()).map_err(|_err| {
                        ArrowError::ArithmeticOverflow(format!(
                            "`.[]` can't handle fixed size list with length {}",
                            fixed.len()
                        ))
                    })?;

                    let (_field, size, values, nulls) = fixed.into_parts();
                    let offsets: Vec<i32> = (0..=len).map(|i| i * size).collect();
                    let offsets = OffsetBuffer::new(ScalarBuffer::from(offsets));

                    // TODO(grtlr): Since we don't keep track that these offsets came from a fixed size
                    // list array we also can't restore it back up in the tree. To fix this we'd have
                    // to make `offsets` an enum to distinguish between containers.
                    Ok(Some(EvalResult {
                        array: values,
                        offsets: Some(offsets),
                        nulls,
                    }))
                }
                dt => Err(ArrowError::InvalidArgumentError(format!(
                    "`.[]` called on unexpected type {dt}"
                )))?,
            },

            Self::Pipe { left, right, .. } => {
                let Some(left_result) = left.eval(source, runtime)? else {
                    return Ok(None);
                };
                let Some(right_result) = right.eval(left_result.array, runtime)? else {
                    return Ok(None);
                };

                Ok(Some(EvalResult {
                    array: right_result.array,
                    offsets: compose_offsets(
                        left_result.offsets.as_ref(),
                        right_result.offsets.as_ref(),
                    ),
                    nulls: combine_null_buffers(
                        left_result.nulls.as_ref(),
                        right_result.nulls.as_ref(),
                    ),
                }))
            }

            // TODO(RR-3435): FixedSizeListArray errors must be suppressed via `?`, but ListArray should not need it.
            Self::Try(inner) => match inner.eval(source, runtime) {
                Ok(result) => Ok(result),
                Err(err) => {
                    re_log::trace!("try expression suppressed error: {err}");
                    Ok(None)
                }
            },

            Self::NonNull(inner) => {
                let Some(result) = inner.eval(source, runtime)? else {
                    return Ok(None);
                };

                Ok(Some(promote_inner_nulls(result)))
            }

            Self::Function { name, arguments } => {
                let function = runtime
                    .function_registry
                    .get(name, arguments.as_ref().map_or(&[], |v| v.as_slice()))?;
                match function(&source)? {
                    Some(result) => Ok(Some(EvalResult::flat(result))),
                    None => Ok(None),
                }
            }

            Self::Map(body) => match source.data_type() {
                DataType::List(_) => {
                    let list_array = source.as_list::<i32>();
                    match eval_map(list_array, body.as_ref(), runtime)? {
                        Some(inner_list_array) => {
                            Ok(Some(EvalResult::flat(Arc::new(inner_list_array))))
                        }
                        None => Ok(None),
                    }
                }
                dt @ DataType::FixedSizeList(..) => Err(ArrowError::NotYetImplemented(format!(
                    "`map()` is not yet implemented for {dt}"
                )))?,
                dt => Err(ArrowError::InvalidArgumentError(format!(
                    "cannot call `.map()` on unexpected type {dt}"
                )))?,
            },
        }
    }
}

impl Eval for DynExpr {
    fn eval(
        &self,
        source: ArrayRef,
        runtime: &Runtime,
    ) -> Result<Option<EvalResult>, crate::combinators::Error> {
        match self {
            Self::Expr(expr) => expr.eval(source, runtime),

            Self::Pipe { left, right } => {
                let Some(left_result) = left.eval(source, runtime)? else {
                    return Ok(None);
                };
                let Some(right_result) = right.eval(left_result.array, runtime)? else {
                    return Ok(None);
                };

                Ok(Some(EvalResult {
                    array: right_result.array,
                    offsets: compose_offsets(
                        left_result.offsets.as_ref(),
                        right_result.offsets.as_ref(),
                    ),
                    nulls: combine_null_buffers(
                        left_result.nulls.as_ref(),
                        right_result.nulls.as_ref(),
                    ),
                }))
            }

            Self::Function(f) => match f(&source)? {
                Some(result) => Ok(Some(EvalResult::flat(result))),
                None => Ok(None),
            },
        }
    }
}
