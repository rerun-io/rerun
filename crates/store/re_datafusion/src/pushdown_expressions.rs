use arrow::datatypes::SchemaRef;
use datafusion::common::{DataFusionError, ScalarValue, exec_err};
use datafusion::logical_expr::{BinaryExpr, Expr, Operator, TableProviderFilterPushDown, lit};
use re_log_types::{AbsoluteTimeRange, TimeInt};
use re_protos::cloud::v1alpha1::ext::{Query, QueryDatasetRequest, QueryLatestAt, QueryRange};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_sorbet::metadata::RERUN_KIND;
use std::ops::Not as _;

fn arrange_binary_expr_as_col_on_left(expr: &BinaryExpr) -> BinaryExpr {
    if let Expr::Column(_) = expr.left.as_ref() {
        return expr.clone();
    }

    let op = match expr.op {
        Operator::Gt => Operator::LtEq,
        Operator::GtEq => Operator::Lt,
        Operator::Lt => Operator::GtEq,
        Operator::LtEq => Operator::Gt,
        _ => expr.op,
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
) -> Result<TableProviderFilterPushDown, DataFusionError> {
    let returned_queries =
        apply_filter_expr_to_queries(vec![query_dataset_request.clone()], filter_expr, schema)?;

    if returned_queries.is_some() {
        Ok(TableProviderFilterPushDown::Inexact)
    } else {
        Ok(TableProviderFilterPushDown::Unsupported)
    }
}

/// Apply a filter expression to a dataset query.
/// This function will return Ok(None) if we cannot push down this filter into our request.
/// It will return an error if the expression pushes down to return no results. This can
/// occur if you have two mutually exclusive expressions that cannot overlap, such as
/// `rerun_segment_id == "aaaa" AND rerun_segment_id == "BBBB"`.
pub(crate) fn apply_filter_expr_to_queries(
    queries: Vec<QueryDatasetRequest>,
    expr: &Expr,
    schema: &SchemaRef,
) -> Result<Option<Vec<QueryDatasetRequest>>, DataFusionError> {
    Ok(match expr {
        Expr::Alias(alias_expr) => {
            apply_filter_expr_to_queries(queries, alias_expr.expr.as_ref(), schema)?
        }
        Expr::BinaryExpr(expr) => {
            let BinaryExpr { left, op, right } = arrange_binary_expr_as_col_on_left(expr);

            match op {
                Operator::And => {
                    // When we have multiple queries they are effectively ORed together.
                    // When we apply the expression to both the left and right we will
                    // have (leftA OR leftB) AND (rightC OR rightD). We need to
                    // consider the combinatorial for the final output.

                    let Some(left_queries) =
                        apply_filter_expr_to_queries(queries.clone(), &left, schema)?
                    else {
                        return Ok(None);
                    };
                    let Some(right_queries) =
                        apply_filter_expr_to_queries(queries.clone(), &right, schema)?
                    else {
                        return Ok(None);
                    };

                    let final_exprs = left_queries
                        .iter()
                        .flat_map(|left| {
                            right_queries
                                .iter()
                                .map(|right| merge_queries_and(left, right))
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    Some(final_exprs)
                }
                Operator::Or => {
                    let Some(mut left_queries) =
                        apply_filter_expr_to_queries(queries.clone(), &left, schema)?
                    else {
                        return Ok(None);
                    };
                    let Some(right_queries) =
                        apply_filter_expr_to_queries(queries.clone(), &right, schema)?
                    else {
                        return Ok(None);
                    };

                    left_queries.extend(right_queries);
                    Some(left_queries)
                }
                Operator::Eq | Operator::Gt | Operator::GtEq | Operator::Lt | Operator::LtEq => {
                    match known_filter_column(left.as_ref(), right.as_ref(), schema) {
                        KnownFilterColumn::Index(index_name, time) => Some(
                            queries
                                .iter()
                                .map(|query| replace_time_in_query(query, &index_name, time, op))
                                .collect::<Result<Vec<_>, _>>()?,
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

            apply_filter_expr_to_queries(queries, &expr, schema)?
        }
        Expr::InList(list_expr) => {
            let expr = list_expr.list.iter().fold(lit(true), |acc, item| {
                acc.or(list_expr.expr.as_ref().clone().eq(item.clone()))
            });
            apply_filter_expr_to_queries(queries, &expr, schema)?
        }
        _ => None,
    })
}

fn is_time_index(col_name: &str, schema: &SchemaRef) -> bool {
    if let Some((_, field)) = schema.fields().find(col_name) {
        if let Some(kind) = field.metadata().get(RERUN_KIND)
            && kind == "index"
        {
            return true;
        }
    }
    false
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
) -> Result<QueryDatasetRequest, DataFusionError> {
    let mut merged = left.clone();
    if !right.segment_ids.is_empty() {
        if left.segment_ids.is_empty() {
            merged.segment_ids = right.segment_ids.clone();
        } else {
            merged
                .segment_ids
                .retain(|id| right.segment_ids.contains(id));
            if left.segment_ids.is_empty() {
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
                    left_query.latest_at = Some(latest_at_from_range(&new_range));
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
) -> Result<QueryDatasetRequest, DataFusionError> {
    let mut query_clone = dataset_query.clone();
    let latest_at = Some(QueryLatestAt {
        index: Some(index.to_owned()),
        at: time,
    });

    let (latest_at, range) = match op {
        Operator::Eq => (latest_at, None),
        Operator::Gt | Operator::GtEq => {
            let range = QueryRange {
                index: index.to_owned(),
                index_range: AbsoluteTimeRange {
                    min: time,
                    max: TimeInt::MAX,
                },
            };
            (latest_at, Some(range))
        }
        Operator::Lt | Operator::LtEq => {
            let range = QueryRange {
                index: index.to_owned(),
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
            columns_always_include_chunk_ids: false,
            columns_always_include_byte_offsets: false,
            columns_always_include_entity_paths: false,
            columns_always_include_static_indexes: false,
            columns_always_include_global_indexes: false,
            columns_always_include_component_indexes: false,
        });
    }

    merge_queries_and(dataset_query, &query_clone)
}

fn latest_at_from_range(range: &QueryRange) -> QueryLatestAt {
    QueryLatestAt {
        index: Some(range.index.clone()),
        at: range.index_range.min,
    }
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
        index: left.index.clone(),
        index_range: AbsoluteTimeRange {
            min: left.index_range.min.max(right.index_range.min),
            max: left.index_range.max.min(right.index_range.max),
        },
    })
}
