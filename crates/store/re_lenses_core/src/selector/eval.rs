//! Evaluation of [`Expr`] and [`DynExpr`] against Arrow arrays.

use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, AsArray as _, BooleanBufferBuilder, FixedSizeListArray, ListArray,
    OffsetSizeTrait,
};
use arrow::buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;

use crate::combinators::{GetField, GetIndexList, Transform as _};

use super::DynExpr;
use super::parser::{Expr, PathExpr};
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

/// Type-level facts about a `pack` path, resolved against the input schema.
struct PathExprType {
    data_type: DataType,

    /// `true` if the path can produce nulls according to the schema.
    nullable: bool,

    /// `true` if the path promotes its nulls to the entry-null channel via `!`.
    acknowledged: bool,
}

/// Resolve the output type, nullability, and `!`-acknowledgment of a [`PathExpr`]
/// against the `input` datatype.
///
/// `root` is the full path, carried through for error messages. Because [`PathExpr`] only
/// models scalar navigation, this match is exhaustive — non-1:1 and dynamically-typed
/// forms were already rejected at parse time, so there is no catch-all branch.
fn resolve_path(
    path: &PathExpr,
    input: &DataType,
    input_nullable: bool,
    root: &PathExpr,
) -> Result<PathExprType, crate::combinators::Error> {
    use crate::combinators::Error;

    match path {
        PathExpr::Identity => Ok(PathExprType {
            data_type: input.clone(),
            nullable: input_nullable,
            acknowledged: false,
        }),

        PathExpr::Field(name) => {
            let DataType::Struct(fields) = input else {
                return Err(Error::TypeMismatch {
                    expected: "Struct".to_owned(),
                    actual: input.clone(),
                    context: format!("`pack` path `{root}` accesses field `.{name}`"),
                });
            };
            let field =
                fields
                    .iter()
                    .find(|f| f.name() == name)
                    .ok_or_else(|| Error::FieldNotFound {
                        field_name: name.clone(),
                        available_fields: fields.iter().map(|f| f.name().clone()).collect(),
                    })?;
            Ok(PathExprType {
                data_type: field.data_type().clone(),
                // Presence propagation: `.a.b` is nullable if `a` (the input) or `b` is.
                nullable: input_nullable || field.is_nullable(),
                acknowledged: false,
            })
        }

        PathExpr::Index(_) => {
            let child = match input {
                DataType::List(field) | DataType::FixedSizeList(field, _) => {
                    field.data_type().clone()
                }
                _ => {
                    return Err(Error::TypeMismatch {
                        expected: "List or FixedSizeList".to_owned(),
                        actual: input.clone(),
                        context: format!("`pack` path `{root}` indexes into a list"),
                    });
                }
            };
            Ok(PathExprType {
                data_type: child,
                // Indexing can be out of bounds, so the result is always potentially null.
                nullable: true,
                acknowledged: false,
            })
        }

        PathExpr::NonNull(inner) => Ok(PathExprType {
            acknowledged: true,
            ..resolve_path(inner, input, input_nullable, root)?
        }),

        // `?` suppresses absence, not nulls, so it does not acknowledge nullability.
        PathExpr::Try(inner) => resolve_path(inner, input, input_nullable, root),

        PathExpr::Pipe {
            left,
            right,
            implicit: _,
        } => {
            let left = resolve_path(left, input, input_nullable, root)?;
            resolve_path(right, &left.data_type, left.nullable, root)
        }
    }
}

/// Evaluate a `pack(path, …, path)` expression into a [`FixedSizeListArray`].
///
/// Each path is evaluated against `source`, and the per-row results are packed into a
/// fixed-size list of size `paths.len()`. See the [module docs](crate::selector) for the full
/// nullability contract (the `!` gate and entry-level AND semantics).
fn eval_pack(
    paths: &[PathExpr],
    source: &ArrayRef,
    runtime: &Runtime,
) -> Result<Option<EvalResult>, crate::combinators::Error> {
    use crate::combinators::Error;

    re_log::debug_assert!(
        !paths.is_empty(),
        "the parser guarantees `pack` has at least one path"
    );

    // --- Type-driven validation (schema-level, before touching any data) ---

    let input_dt = source.data_type();
    // The root input is a bare array with no `Field`, so we can only seed its nullability
    // from the array itself. Field accesses below use the struct's exact field nullability.
    let input_nullable = source.logical_nulls().is_some();

    let mut expected_dt: Option<DataType> = None;
    let mut any_nullable = false;

    for path in paths {
        let resolved = resolve_path(path, input_dt, input_nullable, path)?;

        // The `!` gate: a nullable path must acknowledge its nulls with `!`, since a null
        // shadows the whole entry (AND semantics).
        if resolved.nullable && !resolved.acknowledged {
            return Err(Error::PackPathNullable {
                path: format!("{path}"),
            });
        }
        any_nullable |= resolved.nullable;

        match &expected_dt {
            None => expected_dt = Some(resolved.data_type),
            Some(expected) if *expected != resolved.data_type => {
                return Err(Error::PackPathTypeMismatch {
                    path: format!("{path}"),
                    actual_type: resolved.data_type,
                    expected_type: expected.clone(),
                });
            }
            Some(_) => {}
        }
    }

    // --- Evaluate paths and assemble the FixedSizeList ---

    let mut path_arrays = Vec::with_capacity(paths.len());
    let mut entry_validity: Option<NullBuffer> = None;
    let mut num_rows: Option<usize> = None;

    for path in paths {
        let Some(result) = Expr::from(path).eval(source.clone(), runtime)? else {
            // A path whose error was suppressed (`?`) makes the whole `pack` absent.
            return Ok(None);
        };

        // A path is pure navigation (`PathExpr` excludes iteration), so it is always
        // 1:1 and never introduces offsets.
        re_log::debug_assert!(
            result.offsets.is_none(),
            "`pack` path `{path}` unexpectedly produced offsets despite being a scalar path"
        );

        // The path's promoted nulls (populated by `!`) feed the entry-level AND.
        entry_validity = combine_null_buffers(entry_validity.as_ref(), result.nulls.as_ref());

        match num_rows {
            None => num_rows = Some(result.array.len()),
            Some(n) => re_log::debug_assert_eq!(
                n,
                result.array.len(),
                "all `pack` paths must produce the same number of rows"
            ),
        }

        path_arrays.push(result.array);
    }

    let num_rows = num_rows.unwrap_or(0);
    let num_paths = path_arrays.len();
    let element_type = path_arrays[0].data_type().clone();

    // Build the row-major child buffer `[r0p0, r0p1, …, r1p0, …]` by slicing each path
    // per row and concatenating. This mirrors `StructToFixedList` and avoids needing a
    // dedicated interleave kernel.
    let child = if num_rows == 0 {
        path_arrays[0].slice(0, 0)
    } else {
        let mut slices = Vec::with_capacity(num_rows * num_paths);
        for row in 0..num_rows {
            for path_array in &path_arrays {
                slices.push(path_array.slice(row, 1));
            }
        }
        let refs: Vec<&dyn arrow::array::Array> = slices.iter().map(|a| a.as_ref()).collect();
        re_arrow_util::concat_arrays(&refs)?
    };

    let size = i32::try_from(num_paths).map_err(|err| Error::InvalidNumberOfFields {
        actual: num_paths,
        err,
    })?;

    // The child field is nullable iff any path is nullable. By the AND semantics, an
    // element-level null only ever occurs under a null entry, so consumers that check
    // entry validity never observe element-level nulls.
    let field = Arc::new(Field::new_list_field(element_type, any_nullable));
    let fixed = FixedSizeListArray::new(field, size, child, entry_validity);

    Ok(Some(EvalResult::flat(Arc::new(fixed))))
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
                    .function_registry()
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

            Self::Pack(paths) => eval_pack(paths, &source, runtime),
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
