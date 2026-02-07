use arrow::datatypes::SchemaRef;
use datafusion::common::{DataFusionError, ScalarValue, exec_err};
use datafusion::logical_expr::{BinaryExpr, Expr, Operator, TableProviderFilterPushDown};
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
        | Operator::QuestionPipe => expr.op,
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
                        return apply_filter_expr_to_queries(queries.clone(), &right, schema);
                    };
                    let Some(right_queries) =
                        apply_filter_expr_to_queries(queries.clone(), &right, schema)?
                    else {
                        return Ok(Some(left_queries));
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
                        return Ok(Some(queries));
                    };
                    let Some(right_queries) =
                        apply_filter_expr_to_queries(queries.clone(), &right, schema)?
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
            let mut iter = list_expr.list.iter();
            if let Some(first) = iter.next() {
                let expr = iter.fold(
                    list_expr.expr.as_ref().clone().eq(first.clone()),
                    |acc, item| acc.or(list_expr.expr.as_ref().clone().eq(item.clone())),
                );
                apply_filter_expr_to_queries(queries, &expr, schema)?
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
        Operator::Eq => {
            let range = QueryRange {
                index: index.to_owned(),
                index_range: AbsoluteTimeRange {
                    min: time,
                    max: time,
                },
            };
            (latest_at, Some(range))
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::{Field, Schema};
    use datafusion::logical_expr::expr::InList;
    use datafusion::logical_expr::{col, lit};
    use std::collections::HashMap;
    use std::sync::Arc;

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

    fn make_query_with_segment(segment_id: &str) -> QueryDatasetRequest {
        QueryDatasetRequest {
            segment_ids: vec![segment_id.into()],
            ..Default::default()
        }
    }

    // ==================== Segment ID filter tests ====================

    #[test]
    fn test_segment_id_eq_filter() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("rerun_segment_id").eq(lit("segment_a"));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_segment_id_or_creates_multiple_queries() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("rerun_segment_id")
            .eq(lit("segment_a"))
            .or(col("rerun_segment_id").eq(lit("segment_b")));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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

        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema);

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

        assert!(result.is_none());
    }

    // ==================== Error case tests ====================

    #[test]
    fn test_conflicting_segment_ids_error() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_query_with_segment("segment_a");

        // Try to AND with a different segment
        let expr = col("rerun_segment_id").eq(lit("segment_b"));
        let result = apply_filter_expr_to_queries(vec![query.clone()], &expr, &schema).unwrap();

        // First apply creates query with segment_b
        assert!(result.is_some());
        let queries = result.unwrap();

        // Now try to merge with conflicting segment
        let merge_result = merge_queries_and(&query, &queries[0]);
        assert!(merge_result.is_err());
    }

    #[test]
    fn test_non_overlapping_time_ranges_error() {
        let left = QueryRange {
            index: "frame_nr".to_owned(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "frame_nr".to_owned(),
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
            index: "frame_nr".to_owned(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "log_time".to_owned(),
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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
        let result = filter_expr_is_supported(&expr, &query, &schema).unwrap();

        assert_eq!(result, TableProviderFilterPushDown::Inexact);
    }

    #[test]
    fn test_filter_expr_is_supported_returns_unsupported() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("unknown_column").eq(lit(100i64));
        let result = filter_expr_is_supported(&expr, &query, &schema).unwrap();

        assert_eq!(result, TableProviderFilterPushDown::Unsupported);
    }

    // ==================== Alias expression tests ====================

    #[test]
    fn test_aliased_expression() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("frame_nr").eq(lit(100i64)).alias("my_filter");
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema).unwrap();

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
            index: "frame_nr".to_owned(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "frame_nr".to_owned(),
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
            index: "frame_nr".to_owned(),
            index_range: AbsoluteTimeRange {
                min: TimeInt::new_temporal(0),
                max: TimeInt::new_temporal(100),
            },
        };
        let right = QueryRange {
            index: "frame_nr".to_owned(),
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
