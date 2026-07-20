use arrow::datatypes::{DataType, Schema, SchemaRef};
use datafusion::common::{DataFusionError, ScalarValue, exec_err};
use datafusion::logical_expr::{BinaryExpr, Expr, Operator, TableProviderFilterPushDown};
use itertools::Itertools as _;
use re_log_types::{AbsoluteTimeRange, TimeInt, TimelineName};
use re_protos::cloud::v1alpha1::ext::{Query, QueryDatasetRequest, QueryLatestAt, QueryRange};
use re_sorbet::metadata::RERUN_KIND;
use re_types_core::SegmentId;
use std::collections::HashSet;
use std::ops::Not as _;

/// True if every column `expr` references is in `supported_columns`.
///
/// Used to decide whether a filter can be serialized for server-side pushdown — we only push
/// filters that reference columns the server knows how to parse.
pub(crate) fn expr_columns_supported(expr: &Expr, supported_columns: &HashSet<String>) -> bool {
    expr.column_refs()
        .iter()
        .all(|col| supported_columns.contains(col.name.as_str()))
}

/// True when `expr` only uses constructs that survive the SQL round trip to any server: plain
/// column/literal comparisons, boolean logic, `IN` lists, `BETWEEN`, `LIKE`, casts, and the like.
///
/// Anything else is rejected — most notably scalar functions, which may be client-only UDFs the
/// server cannot parse. The server rejects a filter it can't parse with `InvalidArgument`, which
/// would fail the whole scan; keeping such filters client-side keeps pushdown a pure optimization.
fn expr_shape_supports_pushdown(expr: &Expr) -> bool {
    use datafusion::common::tree_node::{TreeNode as _, TreeNodeRecursion};

    let mut supported = true;
    let walked = expr.apply(|expr| match expr {
        Expr::Alias(_)
        | Expr::Column(_)
        | Expr::Literal(..)
        | Expr::BinaryExpr(_)
        | Expr::Like(_)
        | Expr::Not(_)
        | Expr::IsNotNull(_)
        | Expr::IsNull(_)
        | Expr::IsTrue(_)
        | Expr::IsFalse(_)
        | Expr::IsUnknown(_)
        | Expr::IsNotTrue(_)
        | Expr::IsNotFalse(_)
        | Expr::IsNotUnknown(_)
        | Expr::Negative(_)
        | Expr::Between(_)
        | Expr::Case(_)
        | Expr::Cast(_)
        | Expr::TryCast(_)
        | Expr::InList(_) => Ok(TreeNodeRecursion::Continue),

        _ => {
            supported = false;
            Ok(TreeNodeRecursion::Stop)
        }
    });
    walked.is_ok() && supported
}

/// True when `filter` can be offered to the server: every referenced column is supported *and*
/// the expression shape survives the SQL round trip.
pub(crate) fn expr_supports_pushdown(expr: &Expr, supported_columns: &HashSet<String>) -> bool {
    expr_columns_supported(expr, supported_columns) && expr_shape_supports_pushdown(expr)
}

/// Classify `filters` for [`TableProvider::supports_filters_pushdown`]: pushdown-eligible filters
/// are [`TableProviderFilterPushDown::Inexact`] (serialized for the server but re-applied by
/// DataFusion, so the server pushdown stays a pure optimization), the rest
/// [`TableProviderFilterPushDown::Unsupported`].
///
/// [`TableProvider::supports_filters_pushdown`]: datafusion::catalog::TableProvider::supports_filters_pushdown
pub(crate) fn classify_filters_for_pushdown(
    filters: &[&Expr],
    supported_columns: &HashSet<String>,
) -> Vec<TableProviderFilterPushDown> {
    filters
        .iter()
        .map(|filter| {
            if expr_supports_pushdown(filter, supported_columns) {
                TableProviderFilterPushDown::Inexact
            } else {
                TableProviderFilterPushDown::Unsupported
            }
        })
        .collect()
}

/// The columns of `schema` whose Arrow types are safe candidates for filter pushdown.
///
/// List and binary-like columns are excluded because their SQL literal representation is not
/// currently supported.
pub(crate) fn pushdown_filterable_columns(schema: &Schema) -> HashSet<String> {
    schema
        .fields()
        .iter()
        .filter(|field| {
            !matches!(
                field.data_type(),
                DataType::List(_)
                    | DataType::LargeList(_)
                    | DataType::ListView(_)
                    | DataType::LargeListView(_)
                    | DataType::FixedSizeList(..)
                    | DataType::Binary
                    | DataType::LargeBinary
                    | DataType::FixedSizeBinary(_)
            )
        })
        .map(|field| field.name().clone())
        .collect()
}

/// Combine the pushdown-eligible `filters` (see [`expr_supports_pushdown`]) into a single SQL
/// boolean expression string for a server-side `filter` request field.
///
/// Returns `None` when no filter is eligible or the combined expression can't be unparsed to SQL.
/// Callers should report these filters as [`TableProviderFilterPushDown::Inexact`] so DataFusion
/// re-applies them regardless of what the server does.
pub(crate) fn filters_to_pushdown_sql(
    filters: &[Expr],
    supported_columns: &HashSet<String>,
) -> Option<String> {
    let combined = filters
        .iter()
        .filter(|filter| expr_supports_pushdown(filter, supported_columns))
        .cloned()
        .reduce(|acc, filter| acc.and(filter))?;

    datafusion::sql::unparser::expr_to_sql(&combined)
        .ok()
        .map(|sql| sql.to_string())
}

fn arrange_binary_expr_as_col_on_left(expr: &BinaryExpr) -> BinaryExpr {
    if let Expr::Column(_) = expr.left.as_ref() {
        return expr.clone();
    }

    let op = match expr.op {
        Operator::Gt => Operator::LtEq,
        Operator::GtEq => Operator::Lt,
        Operator::Lt => Operator::GtEq,
        Operator::LtEq => Operator::Gt,

        Operator::Eq
        | Operator::NotEq
        | Operator::Plus
        | Operator::Minus
        | Operator::Multiply
        | Operator::Divide
        | Operator::Modulo
        | Operator::And
        | Operator::Or
        | Operator::IsDistinctFrom
        | Operator::IsNotDistinctFrom
        | Operator::RegexMatch
        | Operator::RegexIMatch
        | Operator::RegexNotMatch
        | Operator::RegexNotIMatch
        | Operator::LikeMatch
        | Operator::ILikeMatch
        | Operator::NotLikeMatch
        | Operator::NotILikeMatch
        | Operator::BitwiseAnd
        | Operator::BitwiseOr
        | Operator::BitwiseXor
        | Operator::BitwiseShiftRight
        | Operator::BitwiseShiftLeft
        | Operator::StringConcat
        | Operator::AtArrow
        | Operator::ArrowAt
        | Operator::Arrow
        | Operator::LongArrow
        | Operator::HashArrow
        | Operator::HashLongArrow
        | Operator::AtAt
        | Operator::IntegerDivide
        | Operator::HashMinus
        | Operator::AtQuestion
        | Operator::Question
        | Operator::QuestionAnd
        | Operator::QuestionPipe
        | Operator::Colon => expr.op,
    };

    BinaryExpr {
        left: Box::new(expr.right.as_ref().clone()),
        op,
        right: Box::new(expr.left.as_ref().clone()),
    }
}

pub(crate) fn filter_expr_is_supported(
    filter_expr: &Expr,
    query_dataset_request: &QueryDatasetRequest,
    schema: &SchemaRef,
    synthesize_latest_at: bool,
) -> Result<TableProviderFilterPushDown, DataFusionError> {
    let returned_queries = apply_filter_expr_to_queries(
        vec![query_dataset_request.clone()],
        filter_expr,
        schema,
        synthesize_latest_at,
    )?;

    if returned_queries.is_some() {
        Ok(TableProviderFilterPushDown::Inexact)
    } else {
        Ok(TableProviderFilterPushDown::Unsupported)
    }
}

/// Apply a filter expression to a dataset query.
///
/// `synthesize_latest_at` controls whether time-index pushdown pairs each
/// `range` with a synthesized `latest_at`. Pass `true` when the caller wants
/// sparse-fill semantics ([`re_dataframe::SparseFillStrategy::LatestAtGlobal`]); pass
/// `false` (the common case, [`re_dataframe::SparseFillStrategy::None`]) to skip the
/// synthesis — it would otherwise force the server into an expensive
/// latest-at fan-out for no observable result.
///
/// This function will return Ok(None) if we cannot push down this filter into our request.
/// It will return an error if the expression pushes down to return no results. This can
/// occur if you have two mutually exclusive expressions that cannot overlap, such as
/// `rerun_segment_id == "aaaa" AND rerun_segment_id == "BBBB"`.
pub(crate) fn apply_filter_expr_to_queries(
    queries: Vec<QueryDatasetRequest>,
    expr: &Expr,
    schema: &SchemaRef,
    synthesize_latest_at: bool,
) -> Result<Option<Vec<QueryDatasetRequest>>, DataFusionError> {
    Ok(match expr {
        Expr::Alias(alias_expr) => apply_filter_expr_to_queries(
            queries,
            alias_expr.expr.as_ref(),
            schema,
            synthesize_latest_at,
        )?,
        Expr::BinaryExpr(expr) => {
            let BinaryExpr { left, op, right } = arrange_binary_expr_as_col_on_left(expr);

            match op {
                Operator::And => {
                    // When we have multiple queries they are effectively ORed together.
                    // When we apply the expression to both the left and right we will
                    // have (leftA OR leftB) AND (rightC OR rightD). We need to
                    // consider the combinatorial for the final output.

                    match (
                        apply_filter_expr_to_queries(
                            queries.clone(),
                            &left,
                            schema,
                            synthesize_latest_at,
                        )?,
                        apply_filter_expr_to_queries(
                            queries,
                            &right,
                            schema,
                            synthesize_latest_at,
                        )?,
                    ) {
                        (None, None) => None,
                        (Some(queries), None) | (None, Some(queries)) => Some(queries),
                        (Some(left_queries), Some(right_queries)) => {
                            let final_exprs = left_queries
                                .iter()
                                .flat_map(|left| {
                                    right_queries.iter().map(|right| {
                                        merge_queries_and(left, right, synthesize_latest_at)
                                    })
                                })
                                .try_collect()?;

                            Some(final_exprs)
                        }
                    }
                }
                Operator::Or => {
                    let Some(mut left_queries) = apply_filter_expr_to_queries(
                        queries.clone(),
                        &left,
                        schema,
                        synthesize_latest_at,
                    )?
                    else {
                        return Ok(Some(queries));
                    };
                    let Some(right_queries) = apply_filter_expr_to_queries(
                        queries.clone(),
                        &right,
                        schema,
                        synthesize_latest_at,
                    )?
                    else {
                        return Ok(Some(queries));
                    };

                    left_queries.extend(right_queries);
                    Some(left_queries)
                }
                Operator::Eq | Operator::Gt | Operator::GtEq | Operator::Lt | Operator::LtEq => {
                    match known_filter_column(left.as_ref(), right.as_ref(), schema) {
                        KnownFilterColumn::Index(index_name, time) => Some(
                            queries
                                .into_iter()
                                .map(|query| {
                                    replace_time_in_query(
                                        &query,
                                        &index_name,
                                        time,
                                        op,
                                        synthesize_latest_at,
                                    )
                                })
                                .try_collect()?,
                        ),
                        KnownFilterColumn::SegmentId(segment_id) => {
                            if op == Operator::Eq {
                                // It is possible to have conflicting queries from before
                                // but this is "good enough" to override since this is a
                                // strict filter, and we will let datafusion finish off the
                                // row filtering later.
                                let returned_queries = queries
                                    .into_iter()
                                    .map(|mut query| {
                                        query.segment_ids = vec![segment_id.clone()];
                                        query
                                    })
                                    .collect::<Vec<_>>();

                                Some(returned_queries)
                            } else {
                                None
                            }
                        }
                        KnownFilterColumn::Unknown => None,
                    }
                }
                _ => None,
            }
        }
        Expr::Between(between_expr) => {
            let left = between_expr
                .expr
                .as_ref()
                .clone()
                .gt_eq(between_expr.low.as_ref().clone());
            let right = between_expr
                .expr
                .as_ref()
                .clone()
                .lt_eq(between_expr.high.as_ref().clone());

            let mut expr = left.and(right);
            if between_expr.negated {
                expr = expr.not();
            }

            apply_filter_expr_to_queries(queries, &expr, schema, synthesize_latest_at)?
        }
        Expr::InList(list_expr) => {
            let mut iter = list_expr.list.iter();
            if let Some(first) = iter.next() {
                let expr = iter.fold(
                    list_expr.expr.as_ref().clone().eq(first.clone()),
                    |acc, item| acc.or(list_expr.expr.as_ref().clone().eq(item.clone())),
                );
                apply_filter_expr_to_queries(queries, &expr, schema, synthesize_latest_at)?
            } else {
                return exec_err!(
                    "Attempting to perform InList statement that would return no results due to empty list"
                );
            }
        }
        _ => None,
    })
}

fn is_time_index(col_name: &str, schema: &SchemaRef) -> bool {
    if let Some((_, field)) = schema.fields().find(col_name)
        && let Some(kind) = field.metadata().get(RERUN_KIND)
        && kind == "index"
    {
        true
    } else {
        false
    }
}

fn expr_to_literal_scalar(expr: &Expr) -> Option<&ScalarValue> {
    match expr {
        Expr::Literal(sv, _) => Some(sv),
        Expr::Alias(e) => expr_to_literal_scalar(e.expr.as_ref()),
        _ => None,
    }
}

enum KnownFilterColumn {
    /// Rerun segment ID
    SegmentId(SegmentId),

    /// Index name and value
    Index(String, TimeInt),

    /// Any other expression
    Unknown,
}

fn known_filter_column(
    column_expr: &Expr,
    value_expr: &Expr,
    schema: &SchemaRef,
) -> KnownFilterColumn {
    if let Expr::Column(col_expr) = column_expr
        && let Some(value) = expr_to_literal_scalar(value_expr)
    {
        if col_expr.name == "rerun_segment_id" {
            let value = match value {
                ScalarValue::Utf8(Some(v))
                | ScalarValue::Utf8View(Some(v))
                | ScalarValue::LargeUtf8(Some(v)) => v.as_str(),
                _ => return KnownFilterColumn::Unknown,
            };
            let segment_id: SegmentId = value.into();
            KnownFilterColumn::SegmentId(segment_id)
        } else if is_time_index(col_expr.name(), schema) {
            let value = match value {
                ScalarValue::UInt8(Some(v)) => *v as i64,
                ScalarValue::UInt16(Some(v)) => *v as i64,
                ScalarValue::UInt32(Some(v)) => *v as i64,
                ScalarValue::UInt64(Some(v)) => i64::try_from(*v).unwrap_or(i64::MAX),
                ScalarValue::Int8(Some(v)) => *v as i64,
                ScalarValue::Int16(Some(v)) => *v as i64,
                ScalarValue::Int32(Some(v)) => *v as i64,
                ScalarValue::Int64(Some(v))
                | ScalarValue::TimestampNanosecond(Some(v), _)
                | ScalarValue::DurationNanosecond(Some(v)) => *v,
                ScalarValue::TimestampSecond(Some(v), _) | ScalarValue::DurationSecond(Some(v)) => {
                    *v * 1_000_000_000
                }
                ScalarValue::TimestampMillisecond(Some(v), _)
                | ScalarValue::DurationMillisecond(Some(v)) => *v * 1_000_000,
                ScalarValue::TimestampMicrosecond(Some(v), _)
                | ScalarValue::DurationMicrosecond(Some(v)) => *v * 1_000,
                _ => return KnownFilterColumn::Unknown,
            };
            let time = TimeInt::new_temporal(value);
            KnownFilterColumn::Index(col_expr.name().to_owned(), time)
        } else {
            KnownFilterColumn::Unknown
        }
    } else {
        KnownFilterColumn::Unknown
    }
}

/// Merge two queries using AND logic. This code only checks for segment IDs and time indexes.
/// If we add additional push down filters, it is vital that this section be updated to account
/// for additional logic.
fn merge_queries_and(
    left: &QueryDatasetRequest,
    right: &QueryDatasetRequest,
    synthesize_latest_at: bool,
) -> Result<QueryDatasetRequest, DataFusionError> {
    let mut merged = left.clone();
    if !right.segment_ids.is_empty() {
        if merged.segment_ids.is_empty() {
            merged.segment_ids = right.segment_ids.clone();
        } else {
            merged
                .segment_ids
                .retain(|id| right.segment_ids.contains(id));
            if merged.segment_ids.is_empty() {
                return exec_err!(
                    "Attempting to perform AND statement that would return no results due to segment ID mismatch"
                );
            }
        }
    }

    if let Some(right_query) = &right.query {
        if let Some(left_query) = &mut merged.query {
            match (
                &left_query.latest_at,
                &left_query.range,
                &right_query.latest_at,
                &right_query.range,
            ) {
                (_, Some(left_range), _, Some(right_range)) => {
                    let new_range = compute_range_overlap(left_range, right_range)?;
                    // Only re-derive `latest_at` from the merged range when
                    // the caller actually wants sparse-fill semantics.
                    left_query.latest_at =
                        synthesize_latest_at.then(|| latest_at_from_range(&new_range));
                    left_query.range = Some(new_range);
                }
                (_, Some(left_range), Some(right_la), None) => {
                    // On the right we want *one* specific time
                    if left_range.index_range.min <= right_la.at
                        && left_range.index_range.max >= right_la.at
                    {
                        left_query.range = None;
                        left_query.latest_at = Some(right_la.clone());
                    } else {
                        exec_err!(
                            "Attempting to merge time range and specific time that are mutually exclusive"
                        )?;
                    }
                }
                (Some(left_la), None, _, Some(right_range)) => {
                    // On the left we want *one* specific time
                    if right_range.index_range.min <= left_la.at
                        && right_range.index_range.max >= left_la.at
                    {
                        // Nothing to do. Left is already limited to one time
                    } else {
                        exec_err!(
                            "Attempting to merge time range and specific time that are mutually exclusive"
                        )?;
                    }
                }
                (Some(left_la), None, Some(right_la), None) => {
                    if left_la.at != right_la.at {
                        exec_err!(
                            "Attempting to merge two specific time requests that are mutually exclusive"
                        )?;
                    }
                }
                (_, _, None, None) => {
                    // Nothing to do
                }
                (None, None, _, _) => {
                    left_query.range = right_query.range.clone();
                    left_query.latest_at = right_query.latest_at.clone();
                }
            }
        } else {
            merged.query = right.query.clone();
        }
    }

    Ok(merged)
}

fn replace_time_in_query(
    dataset_query: &QueryDatasetRequest,
    index: &str,
    time: TimeInt,
    op: Operator,
    synthesize_latest_at: bool,
) -> Result<QueryDatasetRequest, DataFusionError> {
    let mut query_clone = dataset_query.clone();
    let timeline =
        TimelineName::try_new(index).map_err(|err| DataFusionError::External(Box::new(err)))?;
    // `latest_at` is only meaningful to the server when the caller requested
    // sparse-fill semantics. When `synthesize_latest_at` is false, the caller
    // has opted out of fill and any `latest_at` we paired with the range
    // would be dead weight (and an expensive fan-out on the server).
    let synthesized_latest_at =
        synthesize_latest_at.then(|| QueryLatestAt::global(Some(timeline), time));

    let (latest_at, range) = match op {
        Operator::Eq => {
            let range = QueryRange {
                index: timeline,
                index_range: AbsoluteTimeRange {
                    min: time,
                    max: time,
                },
            };
            (synthesized_latest_at, Some(range))
        }
        Operator::Gt | Operator::GtEq => {
            let range = QueryRange {
                index: timeline,
                index_range: AbsoluteTimeRange {
                    min: time,
                    max: TimeInt::MAX,
                },
            };
            (synthesized_latest_at, Some(range))
        }
        Operator::Lt | Operator::LtEq => {
            let range = QueryRange {
                index: timeline,
                index_range: AbsoluteTimeRange {
                    min: TimeInt::MIN,
                    max: time,
                },
            };
            (None, Some(range))
        }
        _ => exec_err!("Invalid operator for merge_time_to_query")?,
    };

    if let Some(query) = &mut query_clone.query {
        query.latest_at = latest_at;
        query.range = range;
    } else {
        query_clone.query = Some(Query {
            latest_at,
            range,
            columns_always_include_everything: false,
            columns_always_include_byte_offsets: false,
            columns_always_include_entity_paths: false,
            columns_always_include_static_indexes: false,
            columns_always_include_global_indexes: false,
            columns_always_include_component_indexes: false,
        });
    }

    merge_queries_and(dataset_query, &query_clone, synthesize_latest_at)
}

fn latest_at_from_range(range: &QueryRange) -> QueryLatestAt {
    QueryLatestAt::global(Some(range.index), range.index_range.min)
}

fn compute_range_overlap(
    left: &QueryRange,
    right: &QueryRange,
) -> Result<QueryRange, DataFusionError> {
    if left.index != right.index {
        return exec_err!("Attempting to compute range overlaps with different indexes");
    }

    if left.index_range.max < right.index_range.min || left.index_range.min > right.index_range.max
    {
        return exec_err!("Attempting to compute range overlaps that are mutually exclusive");
    }

    Ok(QueryRange {
        index: left.index,
        index_range: AbsoluteTimeRange {
            min: left.index_range.min.max(right.index_range.min),
            max: left.index_range.max.min(right.index_range.max),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::{Field, Schema};
    use datafusion::logical_expr::expr::InList;
    use datafusion::logical_expr::{col, lit};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn supported() -> HashSet<String> {
        ["rerun_segment_id".to_owned(), "rerun_layer_name".to_owned()]
            .into_iter()
            .collect()
    }

    #[test]
    fn pushdown_sql_serializes_supported_filters() {
        let filters = vec![col("rerun_segment_id").eq(lit("abc"))];
        let sql = filters_to_pushdown_sql(&filters, &supported()).unwrap();
        assert!(sql.contains("rerun_segment_id"), "got: {sql}");
        assert!(sql.contains("abc"), "got: {sql}");
    }

    #[test]
    fn pushdown_sql_and_multiple_supported_filters() {
        let filters = vec![
            col("rerun_segment_id").eq(lit("abc")),
            col("rerun_layer_name").eq(lit("base")),
        ];
        let sql = filters_to_pushdown_sql(&filters, &supported()).unwrap();
        assert!(sql.contains("rerun_segment_id"), "got: {sql}");
        assert!(sql.contains("rerun_layer_name"), "got: {sql}");
        assert!(sql.to_uppercase().contains("AND"), "got: {sql}");
    }

    #[test]
    fn pushdown_sql_skips_unsupported_columns() {
        // Only the supported filter is serialized; the unsupported one is dropped.
        let filters = vec![
            col("rerun_segment_id").eq(lit("abc")),
            col("rerun_num_chunks").gt(lit(5i64)),
        ];
        let sql = filters_to_pushdown_sql(&filters, &supported()).unwrap();
        assert!(sql.contains("rerun_segment_id"), "got: {sql}");
        assert!(!sql.contains("rerun_num_chunks"), "got: {sql}");
    }

    #[test]
    fn pushdown_sql_none_when_nothing_supported() {
        let filters = vec![col("rerun_num_chunks").gt(lit(5i64))];
        assert!(filters_to_pushdown_sql(&filters, &supported()).is_none());
    }

    #[test]
    fn columns_supported_checks_all_refs() {
        assert!(expr_columns_supported(
            &col("rerun_segment_id").eq(lit("a")),
            &supported()
        ));
        assert!(!expr_columns_supported(
            &col("rerun_segment_id").eq(col("rerun_num_chunks")),
            &supported()
        ));
    }

    #[test]
    fn pushdown_rejects_scalar_functions() {
        // A scalar function may be a client-only UDF the server can't parse — pushing it would
        // fail the whole scan with `InvalidArgument`, so it must stay client-side.
        let expr = datafusion::functions::expr_fn::lower(col("rerun_segment_id")).eq(lit("abc"));
        assert!(!expr_supports_pushdown(&expr, &supported()));
        assert!(filters_to_pushdown_sql(std::slice::from_ref(&expr), &supported()).is_none());
    }

    #[test]
    fn pushdown_accepts_plain_shapes() {
        for expr in [
            col("rerun_segment_id").eq(lit("a")),
            col("rerun_segment_id").in_list(vec![lit("a"), lit("b")], false),
            col("rerun_segment_id").is_not_null(),
            col("rerun_segment_id").like(lit("a%")),
            col("rerun_segment_id").between(lit("a"), lit("b")),
        ] {
            assert!(
                expr_supports_pushdown(&expr, &supported()),
                "expected pushdown support for: {expr:?}"
            );
        }
    }

    #[test]
    fn classify_filters_maps_to_inexact_or_unsupported() {
        let pushable = col("rerun_segment_id").eq(lit("a"));
        let unsupported_column = col("rerun_num_chunks").gt(lit(5i64));
        let unsupported_shape =
            datafusion::functions::expr_fn::lower(col("rerun_segment_id")).eq(lit("a"));

        assert_eq!(
            classify_filters_for_pushdown(
                &[&pushable, &unsupported_column, &unsupported_shape],
                &supported()
            ),
            vec![
                TableProviderFilterPushDown::Inexact,
                TableProviderFilterPushDown::Unsupported,
                TableProviderFilterPushDown::Unsupported,
            ]
        );
    }

    #[test]
    fn filterable_columns_exclude_lists_and_binaries() {
        use arrow::datatypes::Field;

        let schema = Schema::new_with_metadata(
            vec![
                Field::new("id", arrow::datatypes::DataType::Utf8, false),
                Field::new("count", arrow::datatypes::DataType::UInt64, false),
                Field::new(
                    "names",
                    arrow::datatypes::DataType::List(Arc::new(Field::new(
                        "item",
                        arrow::datatypes::DataType::Utf8,
                        true,
                    ))),
                    false,
                ),
                Field::new(
                    "view_names",
                    arrow::datatypes::DataType::ListView(Arc::new(Field::new(
                        "item",
                        arrow::datatypes::DataType::Utf8,
                        true,
                    ))),
                    false,
                ),
                Field::new(
                    "large_view_names",
                    arrow::datatypes::DataType::LargeListView(Arc::new(Field::new(
                        "item",
                        arrow::datatypes::DataType::Utf8,
                        true,
                    ))),
                    false,
                ),
                Field::new(
                    "sha",
                    arrow::datatypes::DataType::FixedSizeBinary(32),
                    false,
                ),
            ],
            HashMap::default(),
        );

        let columns = pushdown_filterable_columns(&schema);
        assert!(columns.contains("id"));
        assert!(columns.contains("count"));
        assert!(!columns.contains("names"));
        assert!(!columns.contains("view_names"));
        assert!(!columns.contains("large_view_names"));
        assert!(!columns.contains("sha"));
    }

    fn make_schema_with_index(index_name: &str) -> SchemaRef {
        let mut metadata = HashMap::new();
        metadata.insert(RERUN_KIND.to_owned(), "index".to_owned());

        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new(index_name, arrow::datatypes::DataType::Int64, false)
                    .with_metadata(metadata),
                Field::new("rerun_segment_id", arrow::datatypes::DataType::Utf8, false),
            ],
            HashMap::default(),
        ))
    }

    fn make_empty_query() -> QueryDatasetRequest {
        QueryDatasetRequest::default()
    }

    fn make_query_with_segment(segment_id: &SegmentId) -> QueryDatasetRequest {
        QueryDatasetRequest {
            segment_ids: vec![segment_id.clone()],
            ..Default::default()
        }
    }

    // ==================== Segment ID filter tests ====================

    #[test]
    fn test_segment_id_eq_filter() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("rerun_segment_id").eq(lit("segment_a"));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].segment_ids.len(), 1);
        assert_eq!(queries[0].segment_ids[0].to_string(), "segment_a");
    }

    #[test]
    fn test_segment_id_gt_not_supported() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        // Greater than on segment_id is not supported
        let expr = col("rerun_segment_id").gt(lit("segment_a"));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_segment_id_or_creates_multiple_queries() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("rerun_segment_id")
            .eq(lit("segment_a"))
            .or(col("rerun_segment_id").eq(lit("segment_b")));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 2);

        // Each query should have exactly one segment ID
        for query in &queries {
            assert_eq!(
                query.segment_ids.len(),
                1,
                "Each query should have exactly one segment ID"
            );
        }

        // Collect all segment IDs
        let segment_ids: Vec<_> = queries
            .iter()
            .map(|q| q.segment_ids[0].to_string())
            .collect();

        // Verify exactly the expected segment IDs are present (no duplicates, no extras)
        assert_eq!(segment_ids.len(), 2);
        assert!(segment_ids.contains(&"segment_a".to_owned()));
        assert!(segment_ids.contains(&"segment_b".to_owned()));

        // Verify no time queries were added
        for query in &queries {
            assert!(
                query.query.is_none(),
                "No time query should be present for segment-only filter"
            );
        }
    }

    // ==================== Time index filter tests ====================

    #[test]
    fn test_time_index_eq_filter() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("frame_nr").eq(lit(100i64));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        let q = queries[0].query.as_ref().unwrap();
        assert!(q.latest_at.is_some());
        assert_eq!(q.latest_at.as_ref().unwrap().at.as_i64(), 100);
        assert!(q.range.is_some());
        let range = q.range.as_ref().unwrap();
        assert_eq!(range.index_range.min.as_i64(), 100);
        assert_eq!(range.index_range.min.as_i64(), 100);
    }

    #[test]
    fn test_time_index_gt_filter() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("frame_nr").gt(lit(100i64));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        let q = queries[0].query.as_ref().unwrap();
        assert!(q.range.is_some());
        let range = q.range.as_ref().unwrap();
        assert_eq!(range.index_range.min.as_i64(), 100);
        assert_eq!(range.index_range.max, TimeInt::MAX);
    }

    #[test]
    fn test_time_index_lt_filter() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("frame_nr").lt(lit(100i64));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        let q = queries[0].query.as_ref().unwrap();
        assert!(q.range.is_some());
        let range = q.range.as_ref().unwrap();
        assert_eq!(range.index_range.min, TimeInt::MIN);
        assert_eq!(range.index_range.max.as_i64(), 100);
    }

    #[test]
    fn test_time_index_range_filter() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        // frame_nr >= 50 AND frame_nr <= 150
        let expr = col("frame_nr")
            .gt_eq(lit(50i64))
            .and(col("frame_nr").lt_eq(lit(150i64)));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        let q = queries[0].query.as_ref().unwrap();
        assert!(q.range.is_some());
        let range = q.range.as_ref().unwrap();
        assert_eq!(range.index_range.min.as_i64(), 50);
        assert_eq!(range.index_range.max.as_i64(), 150);
    }

    #[test]
    fn test_reversed_comparison_order() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        // 100 < frame_nr should be rearranged to frame_nr > 100
        let expr = lit(100i64).lt(col("frame_nr"));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        let q = queries[0].query.as_ref().unwrap();
        assert!(q.range.is_some());
        let range = q.range.as_ref().unwrap();
        // lt becomes gt when reversed, so min should be 100
        assert_eq!(range.index_range.min.as_i64(), 100);
    }

    // ==================== Combined filter tests ====================

    #[test]
    fn test_segment_and_time_filter() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("rerun_segment_id")
            .eq(lit("segment_a"))
            .and(col("frame_nr").gt(lit(100i64)));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].segment_ids[0].to_string(), "segment_a");
        let q = queries[0].query.as_ref().unwrap();
        assert!(q.range.is_some());
        assert_eq!(q.range.as_ref().unwrap().index_range.min.as_i64(), 100);
    }

    #[test]
    fn test_or_with_different_segments_and_times() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        // (segment_a AND frame_nr > 100) OR (segment_b AND frame_nr < 50)
        let expr = col("rerun_segment_id")
            .eq(lit("segment_a"))
            .and(col("frame_nr").gt(lit(100i64)))
            .or(col("rerun_segment_id")
                .eq(lit("segment_b"))
                .and(col("frame_nr").lt(lit(50i64))));

        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 2, "Should produce exactly 2 queries for OR");

        // Each query should have exactly one segment ID
        for query in &queries {
            assert_eq!(
                query.segment_ids.len(),
                1,
                "Each query should have exactly one segment ID"
            );
        }

        // Collect segment IDs and verify no duplicates/extras
        let segment_ids: Vec<_> = queries
            .iter()
            .map(|q| q.segment_ids[0].to_string())
            .collect();
        assert_eq!(segment_ids.len(), 2);
        assert!(segment_ids.contains(&"segment_a".to_owned()));
        assert!(segment_ids.contains(&"segment_b".to_owned()));

        // Find each query by its segment ID
        let query_a = queries
            .iter()
            .find(|q| q.segment_ids[0].to_string() == "segment_a")
            .expect("segment_a query should exist");
        let query_b = queries
            .iter()
            .find(|q| q.segment_ids[0].to_string() == "segment_b")
            .expect("segment_b query should exist");

        // Verify segment_a query: frame_nr > 100 (range from 100 to MAX)
        let q_a = query_a
            .query
            .as_ref()
            .expect("segment_a should have a time query");
        let range_a = q_a.range.as_ref().expect("segment_a should have a range");
        assert_eq!(range_a.index, "frame_nr");
        assert_eq!(range_a.index_range.min.as_i64(), 100);
        assert_eq!(range_a.index_range.max, TimeInt::MAX);

        // Verify segment_b query: frame_nr < 50 (range from MIN to 50)
        let q_b = query_b
            .query
            .as_ref()
            .expect("segment_b should have a time query");
        let range_b = q_b.range.as_ref().expect("segment_b should have a range");
        assert_eq!(range_b.index, "frame_nr");
        assert_eq!(range_b.index_range.min, TimeInt::MIN);
        assert_eq!(range_b.index_range.max.as_i64(), 50);
    }

    // ==================== Between expression tests ====================

    #[test]
    fn test_between_expression() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = Expr::Between(datafusion::logical_expr::Between {
            expr: Box::new(col("frame_nr")),
            negated: false,
            low: Box::new(lit(50i64)),
            high: Box::new(lit(150i64)),
        });
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        let q = queries[0].query.as_ref().unwrap();
        assert!(q.range.is_some());
        let range = q.range.as_ref().unwrap();
        assert_eq!(range.index_range.min.as_i64(), 50);
        assert_eq!(range.index_range.max.as_i64(), 150);
    }

    // ==================== InList expression tests ====================

    #[test]
    fn test_in_list_segment_ids() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = Expr::InList(InList {
            expr: Box::new(col("rerun_segment_id")),
            list: vec![lit("segment_a"), lit("segment_b"), lit("segment_c")],
            negated: false,
        });
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        // InList is converted to OR of equality checks
        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(
            queries.len(),
            3,
            "Should produce 3 queries for IN LIST with 3 items"
        );

        // Each query should have exactly one segment ID
        for query in &queries {
            assert_eq!(query.segment_ids.len(), 1);
        }

        // Verify all segment IDs are present
        let segment_ids: Vec<_> = queries
            .iter()
            .map(|q| q.segment_ids[0].to_string())
            .collect();
        assert!(segment_ids.contains(&"segment_a".to_owned()));
        assert!(segment_ids.contains(&"segment_b".to_owned()));
        assert!(segment_ids.contains(&"segment_c".to_owned()));
    }

    #[test]
    fn test_in_list_time_values() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = Expr::InList(InList {
            expr: Box::new(col("frame_nr")),
            list: vec![lit(100i64), lit(200i64)],
            negated: false,
        });
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        // InList on time values produces OR of equality checks
        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(
            queries.len(),
            2,
            "Should produce 2 queries for IN LIST with 2 items"
        );

        // Each query should have a latest_at time (equality on time = latest_at)
        let times: Vec<_> = queries
            .iter()
            .map(|q| {
                q.query
                    .as_ref()
                    .unwrap()
                    .latest_at
                    .as_ref()
                    .unwrap()
                    .at
                    .as_i64()
            })
            .collect();
        assert!(times.contains(&100));
        assert!(times.contains(&200));
    }

    #[test]
    fn test_in_list_empty() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = Expr::InList(InList {
            expr: Box::new(col("rerun_segment_id")),
            list: vec![],
            negated: false,
        });
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true);

        // Empty list produces zero results
        assert!(result.is_err());
    }

    #[test]
    fn test_in_list_single_item() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = Expr::InList(InList {
            expr: Box::new(col("rerun_segment_id")),
            list: vec![lit("only_segment")],
            negated: false,
        });
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].segment_ids.len(), 1);
        assert_eq!(queries[0].segment_ids[0].to_string(), "only_segment");
    }

    // ==================== Unsupported filter tests ====================

    #[test]
    fn test_unknown_column_not_supported() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("unknown_column").eq(lit(100i64));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_non_index_column_not_supported() {
        // Schema without the index metadata
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new(
                "some_column",
                arrow::datatypes::DataType::Int64,
                false,
            )],
            HashMap::default(),
        ));
        let query = make_empty_query();

        let expr = col("some_column").eq(lit(100i64));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_none());
    }

    // ==================== Error case tests ====================

    #[test]
    fn test_conflicting_segment_ids_error() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_query_with_segment(&SegmentId::from("segment_a"));

        // Try to AND with a different segment
        let expr = col("rerun_segment_id").eq(lit("segment_b"));
        let result =
            apply_filter_expr_to_queries(vec![query.clone()], &expr, &schema, true).unwrap();

        // First apply creates query with segment_b
        assert!(result.is_some());
        let queries = result.unwrap();

        // Now try to merge with conflicting segment
        let merge_result = merge_queries_and(&query, &queries[0], true);
        assert!(merge_result.is_err());
    }

    #[test]
    fn test_non_overlapping_time_ranges_error() {
        let left = QueryRange {
            index: "frame_nr".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "frame_nr".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(200),
                max: TimeInt::new_temporal(300),
            },
        };

        let result = compute_range_overlap(&left, &right);
        assert!(result.is_err());
    }

    #[test]
    fn test_different_index_names_error() {
        let left = QueryRange {
            index: "frame_nr".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "log_time".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(50),
                max: TimeInt::new_temporal(150),
            },
        };

        let result = compute_range_overlap(&left, &right);
        assert!(result.is_err());
    }

    // ==================== Scalar type conversion tests ====================

    #[test]
    fn test_timestamp_nanosecond_conversion() {
        let schema = make_schema_with_index("log_time");
        let query = make_empty_query();

        let expr = col("log_time").eq(Expr::Literal(
            ScalarValue::TimestampNanosecond(Some(1_000_000_000), None),
            None,
        ));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        let q = queries[0].query.as_ref().unwrap();
        assert_eq!(q.latest_at.as_ref().unwrap().at.as_i64(), 1_000_000_000);
    }

    #[test]
    fn test_timestamp_millisecond_conversion() {
        let schema = make_schema_with_index("log_time");
        let query = make_empty_query();

        let expr = col("log_time").eq(Expr::Literal(
            ScalarValue::TimestampMillisecond(Some(1000), None),
            None,
        ));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        let q = queries[0].query.as_ref().unwrap();
        // 1000 ms = 1_000_000_000 ns
        assert_eq!(q.latest_at.as_ref().unwrap().at.as_i64(), 1_000_000_000);
    }

    #[test]
    fn test_timestamp_second_conversion() {
        let schema = make_schema_with_index("log_time");
        let query = make_empty_query();

        let expr = col("log_time").eq(Expr::Literal(
            ScalarValue::TimestampSecond(Some(1), None),
            None,
        ));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        let q = queries[0].query.as_ref().unwrap();
        // 1 s = 1_000_000_000 ns
        assert_eq!(q.latest_at.as_ref().unwrap().at.as_i64(), 1_000_000_000);
    }

    // ==================== filter_expr_is_supported tests ====================

    #[test]
    fn test_filter_expr_is_supported_returns_inexact() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("frame_nr").eq(lit(100i64));
        let result = filter_expr_is_supported(&expr, &query, &schema, true).unwrap();

        assert_eq!(result, TableProviderFilterPushDown::Inexact);
    }

    #[test]
    fn test_filter_expr_is_supported_returns_unsupported() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("unknown_column").eq(lit(100i64));
        let result = filter_expr_is_supported(&expr, &query, &schema, true).unwrap();

        assert_eq!(result, TableProviderFilterPushDown::Unsupported);
    }

    // ==================== Alias expression tests ====================

    #[test]
    fn test_aliased_expression() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("frame_nr").eq(lit(100i64)).alias("my_filter");
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true).unwrap();

        assert!(result.is_some());
        let queries = result.unwrap();
        assert_eq!(queries.len(), 1);
        let q = queries[0].query.as_ref().unwrap();
        assert_eq!(q.latest_at.as_ref().unwrap().at.as_i64(), 100);
    }

    // ==================== Range overlap tests ====================

    #[test]
    fn test_compute_range_overlap_success() {
        let left = QueryRange {
            index: "frame_nr".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "frame_nr".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(50),
                max: TimeInt::new_temporal(150),
            },
        };

        let result = compute_range_overlap(&left, &right).unwrap();
        assert_eq!(result.index, "frame_nr");
        assert_eq!(result.index_range.min.as_i64(), 50);
        assert_eq!(result.index_range.max.as_i64(), 100);
    }

    #[test]
    fn test_compute_range_overlap_touching_boundaries() {
        let left = QueryRange {
            index: "frame_nr".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "frame_nr".into(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(100),
                max: TimeInt::new_temporal(200),
            },
        };

        let result = compute_range_overlap(&left, &right).unwrap();
        assert_eq!(result.index_range.min.as_i64(), 100);
        assert_eq!(result.index_range.max.as_i64(), 100);
    }
}
