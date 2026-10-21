use arrow::datatypes::SchemaRef;
use datafusion::common::{DataFusionError, ScalarValue, exec_err};
use datafusion::logical_expr::{BinaryExpr, Expr, Operator, TableProviderFilterPushDown};
use itertools::Itertools as _;
use re_int::SaturatingCast as _;
use re_log_types::{AbsoluteTimeRange, TimeInt, TimelineName};
use re_protos::cloud::v1alpha1::ext::{Query, QueryDatasetRequest, QueryLatestAt, QueryRange};
use re_protos::cloud::v1alpha1::{SegmentIdFilter, SegmentIdList, segment_id_filter};
use re_sorbet::metadata::RERUN_KIND;
use re_types_core::SegmentId;
use std::collections::HashSet;
use std::ops::Not as _;

#[derive(Clone, Debug, PartialEq, Eq)]
enum SegmentIdSelection {
    ScanOnly(HashSet<String>),
    Skip(HashSet<String>),
}

impl SegmentIdSelection {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::ScanOnly(left), Self::ScanOnly(right)) => Self::ScanOnly(&left & &right),
            (Self::ScanOnly(left), Self::Skip(right))
            | (Self::Skip(right), Self::ScanOnly(left)) => Self::ScanOnly(&left - &right),
            (Self::Skip(left), Self::Skip(right)) => Self::Skip(&left | &right),
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::ScanOnly(left), Self::ScanOnly(right)) => Self::ScanOnly(&left | &right),
            (Self::ScanOnly(left), Self::Skip(right)) => Self::Skip(&right - &left),
            (Self::Skip(left), Self::ScanOnly(right)) => Self::Skip(&left - &right),
            (Self::Skip(left), Self::Skip(right)) => Self::Skip(&left & &right),
        }
    }

    fn negate(self) -> Self {
        match self {
            Self::ScanOnly(ids) => Self::Skip(ids),
            Self::Skip(ids) => Self::ScanOnly(ids),
        }
    }

    fn into_proto(self) -> SegmentIdFilter {
        let strategy = match self {
            Self::ScanOnly(ids) => segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                segment_ids: sorted(ids),
            }),
            Self::Skip(ids) => segment_id_filter::Strategy::Skip(SegmentIdList {
                segment_ids: sorted(ids),
            }),
        };
        SegmentIdFilter {
            strategy: Some(strategy),
        }
    }
}

fn sorted(ids: HashSet<String>) -> Vec<String> {
    ids.into_iter().sorted().collect()
}

fn segment_id_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(
            ScalarValue::Utf8(Some(value))
            | ScalarValue::Utf8View(Some(value))
            | ScalarValue::LargeUtf8(Some(value)),
            _,
        ) => Some(value.clone()),
        _ => None,
    }
}

fn is_segment_id_column(expr: &Expr, column_name: &str) -> bool {
    matches!(expr, Expr::Column(column) if column.name == column_name)
}

fn segment_id_selection(expr: &Expr, column_name: &str) -> Option<SegmentIdSelection> {
    match expr {
        Expr::BinaryExpr(binary) if matches!(binary.op, Operator::Eq | Operator::NotEq) => {
            let value = if is_segment_id_column(&binary.left, column_name) {
                segment_id_literal(&binary.right)
            } else if is_segment_id_column(&binary.right, column_name) {
                segment_id_literal(&binary.left)
            } else {
                None
            }?;
            let ids = HashSet::from([value]);
            Some(if binary.op == Operator::Eq {
                SegmentIdSelection::ScanOnly(ids)
            } else {
                SegmentIdSelection::Skip(ids)
            })
        }

        Expr::BinaryExpr(binary) if matches!(binary.op, Operator::And | Operator::Or) => {
            let left = segment_id_selection(&binary.left, column_name)?;
            let right = segment_id_selection(&binary.right, column_name)?;
            Some(if binary.op == Operator::And {
                left.and(right)
            } else {
                left.or(right)
            })
        }

        Expr::InList(in_list) if is_segment_id_column(&in_list.expr, column_name) => {
            let ids = in_list
                .list
                .iter()
                .map(segment_id_literal)
                .collect::<Option<HashSet<_>>>()?;
            Some(if in_list.negated {
                SegmentIdSelection::Skip(ids)
            } else {
                SegmentIdSelection::ScanOnly(ids)
            })
        }

        Expr::Not(inner) => Some(segment_id_selection(inner, column_name)?.negate()),

        _ => None,
    }
}

/// Convert all representable segment-ID filters into one best-effort server-side scan hint.
pub(crate) fn segment_id_filter_from_filters(
    filters: &[Expr],
    column_name: &str,
) -> Option<SegmentIdFilter> {
    filters
        .iter()
        .filter_map(|filter| segment_id_selection(filter, column_name))
        .reduce(SegmentIdSelection::and)
        .map(SegmentIdSelection::into_proto)
}

/// Classify filters that can be represented by [`SegmentIdFilter`].
pub(crate) fn classify_segment_id_filters_for_pushdown(
    filters: &[&Expr],
    column_name: &str,
) -> Vec<TableProviderFilterPushDown> {
    filters
        .iter()
        .map(|filter| {
            if segment_id_selection(filter, column_name).is_some() {
                // Keep this Inexact while clients can connect to servers that predate
                // `SegmentIdFilter` and silently ignore its unknown protobuf field. This can become
                // Exact after capability negotiation or the compatibility window ends.
                TableProviderFilterPushDown::Inexact
            } else {
                TableProviderFilterPushDown::Unsupported
            }
        })
        .collect()
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

/// The reader-configuration facts the selection-timeline rule
/// ([`choose_selection_timeline`]) depends on, beyond the filters and the request.
#[derive(Default)]
pub struct SelectionContext {
    /// The reader's own timeline (`T`). `None` for a static reader.
    pub reader_index: Option<TimelineName>,

    /// `fill_latest_at`.
    pub fill: bool,

    /// Whether the reader selects specific index values
    /// (`using_index_values`) rather than a plain range.
    pub uses_samples: bool,
}

/// Chooses the timeline Hub or the OSS server uses to select chunks for the SDK to fetch.
///
/// The reader index aligns values into output rows. The selection timeline controls which
/// chunks are fetched before those rows are built. Choosing a different selection timeline
/// does not change the reader index.
///
/// A static reader has no selection timeline. For a temporal reader, use its index when the
/// request has a range or the reader uses fill or sample times. Otherwise, use the first timeline
/// on which a filter can push a server-side range, checking the reader index first and then the
/// other index timelines in expression order. Use the reader index if none qualifies. Fill and
/// sample times require chunks that contribute to rows on the reader index.
///
/// Selecting chunks on a timeline different from the reader index can omit values from entities
/// that did not log that timeline. This pre-existing `QueryDataset` defect also affects
/// `FindChunks` (RR-5885).
///
/// The SDK first chooses a selection timeline in `supports_filters_pushdown` and tells
/// DataFusion which filters it can push down. DataFusion then calls the SDK's `scan` method
/// to build the server requests. It passes only those filters to `scan` and applies the others
/// to the returned rows. Both SDK methods call this function to choose the selection timeline.
///
/// A filter selects a timeline only if it can push a server-side range on it. This ensures
/// DataFusion passes the filter to `scan`, so the request uses the same selection timeline as
/// the earlier pushdown decision. This holds for the reader index too: `scan` never sees a
/// filter that compares the reader index but cannot be pushed as a whole.
pub fn choose_selection_timeline(
    context: &SelectionContext,
    filters: &[&Expr],
    query_dataset_request: &QueryDatasetRequest,
    schema: &SchemaRef,
) -> Result<Option<TimelineName>, DataFusionError> {
    let Some(reader_index) = context.reader_index else {
        return Ok(None);
    };

    let request_restricts_reader = query_dataset_request
        .query
        .as_ref()
        .is_some_and(|query| query.range.is_some());
    if request_restricts_reader || context.fill || context.uses_samples {
        return Ok(Some(reader_index));
    }

    // The reader index comes first, so a pushable restriction on it wins.
    let mut candidate_timelines = vec![reader_index];
    for filter in filters {
        collect_index_timelines(filter, schema, &mut candidate_timelines);
    }

    for timeline in candidate_timelines {
        for filter in filters {
            if pushes_range_on(
                filter,
                timeline,
                query_dataset_request,
                schema,
                context.fill,
            )? {
                return Ok(Some(timeline));
            }
        }
    }

    Ok(Some(reader_index))
}

/// Whether pushing `filter` with `timeline` as the selection timeline puts a range on `timeline`
/// into the request.
fn pushes_range_on(
    filter: &Expr,
    timeline: TimelineName,
    query_dataset_request: &QueryDatasetRequest,
    schema: &SchemaRef,
    fill: bool,
) -> Result<bool, DataFusionError> {
    let queries = apply_filter_expr_to_queries(
        vec![query_dataset_request.clone()],
        filter,
        schema,
        fill,
        Some(timeline),
    )?;
    Ok(queries.is_some_and(|queries| {
        queries.iter().any(|query| {
            query
                .query
                .as_ref()
                .and_then(|query| query.range.as_ref())
                .is_some_and(|range| range.index == timeline)
        })
    }))
}

/// Append the index timelines a filter's leaves compare against, in expression order.
///
/// Structural only, unlike [`known_filter_column`]: it never gates on a
/// selection timeline, because it exists solely to pick one (via
/// [`choose_selection_timeline`]) before any filter is classified. Segment-id
/// and unrecognized comparisons contribute nothing.
fn collect_index_timelines(expr: &Expr, schema: &SchemaRef, timelines: &mut Vec<TimelineName>) {
    match expr {
        Expr::Alias(alias_expr) => collect_index_timelines(&alias_expr.expr, schema, timelines),
        Expr::Not(inner) => collect_index_timelines(inner, schema, timelines),
        Expr::BinaryExpr(binary) if matches!(binary.op, Operator::And | Operator::Or) => {
            collect_index_timelines(&binary.left, schema, timelines);
            collect_index_timelines(&binary.right, schema, timelines);
        }
        Expr::BinaryExpr(binary)
            if matches!(
                binary.op,
                Operator::Eq | Operator::Gt | Operator::GtEq | Operator::Lt | Operator::LtEq
            ) =>
        {
            for timeline in std::iter::chain(
                column_index_timeline(&binary.left, schema),
                column_index_timeline(&binary.right, schema),
            ) {
                if !timelines.contains(&timeline) {
                    timelines.push(timeline);
                }
            }
        }
        Expr::Between(between_expr) => {
            if let Some(timeline) = column_index_timeline(&between_expr.expr, schema)
                && !timelines.contains(&timeline)
            {
                timelines.push(timeline);
            }
        }
        Expr::InList(in_list) => {
            if let Some(timeline) = column_index_timeline(&in_list.expr, schema)
                && !timelines.contains(&timeline)
            {
                timelines.push(timeline);
            }
        }
        _ => {}
    }
}

/// The index timeline `expr` names, if it is a column with `rerun:kind = "index"`.
fn column_index_timeline(expr: &Expr, schema: &SchemaRef) -> Option<TimelineName> {
    let Expr::Column(col) = expr else {
        return None;
    };
    is_time_index(col.name(), schema)
        .then(|| TimelineName::try_new(col.name()).ok())
        .flatten()
}

pub(crate) fn filter_expr_is_supported(
    filter_expr: &Expr,
    query_dataset_request: &QueryDatasetRequest,
    schema: &SchemaRef,
    synthesize_latest_at: bool,
    selection_timeline: Option<TimelineName>,
) -> Result<TableProviderFilterPushDown, DataFusionError> {
    let returned_queries = apply_filter_expr_to_queries(
        vec![query_dataset_request.clone()],
        filter_expr,
        schema,
        synthesize_latest_at,
        selection_timeline,
    )?;

    if returned_queries.is_some() {
        Ok(TableProviderFilterPushDown::Inexact)
    } else {
        Ok(TableProviderFilterPushDown::Unsupported)
    }
}

/// Apply a filter expression to a dataset query.
///
/// An `OR` (or `IN`) of index ranges expands into several queries — one per
/// disjoint branch — which the scan issues as separate, concurrent
/// `query_dataset` requests; `AND` intersects branches rather than adding them.
///
/// `synthesize_latest_at` controls whether time-index pushdown pairs each
/// `range` with a synthesized `latest_at`. Pass `true` when the caller wants
/// sparse-fill semantics ([`re_dataframe::SparseFillStrategy::LatestAtGlobal`]); pass
/// `false` (the common case, [`re_dataframe::SparseFillStrategy::None`]) to skip the
/// synthesis — it would otherwise force the server into an expensive
/// latest-at fan-out for no observable result.
///
/// `selection_timeline` is the request's single selection timeline, chosen by
/// [`choose_selection_timeline`] before applying filters. Comparisons on other
/// index timelines stay in DataFusion for row-wise evaluation, just like
/// comparisons on unrecognized columns. This keeps [`merge_queries_and`] from
/// having to reconcile ranges on different timelines.
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
    selection_timeline: Option<TimelineName>,
) -> Result<Option<Vec<QueryDatasetRequest>>, DataFusionError> {
    Ok(match expr {
        Expr::Alias(alias_expr) => apply_filter_expr_to_queries(
            queries,
            alias_expr.expr.as_ref(),
            schema,
            synthesize_latest_at,
            selection_timeline,
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
                            selection_timeline,
                        )?,
                        apply_filter_expr_to_queries(
                            queries,
                            &right,
                            schema,
                            synthesize_latest_at,
                            selection_timeline,
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
                    // Fan out only when *every* branch is a safe candidate
                    // superset (a segment predicate, or a restriction on
                    // `selection_timeline`). A branch that cannot be pushed leaves the
                    // whole `OR` residual rather than silently widening to an
                    // unfiltered superset, so a mixed-timeline `OR` never intersects
                    // candidate sets from different timelines.
                    match (
                        apply_filter_expr_to_queries(
                            queries.clone(),
                            &left,
                            schema,
                            synthesize_latest_at,
                            selection_timeline,
                        )?,
                        apply_filter_expr_to_queries(
                            queries,
                            &right,
                            schema,
                            synthesize_latest_at,
                            selection_timeline,
                        )?,
                    ) {
                        (Some(mut left_queries), Some(right_queries)) => {
                            left_queries.extend(right_queries);
                            Some(left_queries)
                        }
                        _ => None,
                    }
                }
                Operator::Eq | Operator::Gt | Operator::GtEq | Operator::Lt | Operator::LtEq => {
                    match known_filter_column(
                        left.as_ref(),
                        right.as_ref(),
                        schema,
                        selection_timeline,
                    ) {
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

            apply_filter_expr_to_queries(
                queries,
                &expr,
                schema,
                synthesize_latest_at,
                selection_timeline,
            )?
        }
        Expr::InList(list_expr) => {
            let mut iter = list_expr.list.iter();
            if let Some(first) = iter.next() {
                let expr = iter.fold(
                    list_expr.expr.as_ref().clone().eq(first.clone()),
                    |acc, item| acc.or(list_expr.expr.as_ref().clone().eq(item.clone())),
                );
                apply_filter_expr_to_queries(
                    queries,
                    &expr,
                    schema,
                    synthesize_latest_at,
                    selection_timeline,
                )?
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
    selection_timeline: Option<TimelineName>,
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
            // Only a restriction on the request's one selection
            // timeline is safe to push. A restriction on any other index
            // (including every index when the reader is static) is treated
            // like an unrecognized column and stays residual.
            if selection_timeline.map(|timeline| timeline.as_str()) != Some(col_expr.name()) {
                return KnownFilterColumn::Unknown;
            }
            let value = match value {
                ScalarValue::UInt8(Some(v)) => *v as i64,
                ScalarValue::UInt16(Some(v)) => *v as i64,
                ScalarValue::UInt32(Some(v)) => *v as i64,
                ScalarValue::UInt64(Some(v)) => v.saturating_cast::<i64>(),
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
            merged.segment_ids.clone_from(&right.segment_ids);
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
                    left_query.range.clone_from(&right_query.range);
                    left_query.latest_at.clone_from(&right_query.latest_at);
                }
            }
        } else {
            merged.query.clone_from(&right.query);
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
    // Pushdown keeps only ranges on the selection timeline. The error return keeps a release
    // build from merging two timelines.
    re_log::debug_assert_eq!(
        left.index,
        right.index,
        "pushed ranges must all be on the selection timeline"
    );
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

    const SEGMENT_ID: &str = "rerun_segment_id";

    fn strategy(expr: Expr) -> segment_id_filter::Strategy {
        segment_id_filter_from_filters(&[expr], SEGMENT_ID)
            .unwrap()
            .strategy
            .unwrap()
    }

    #[test]
    fn segment_id_equality_becomes_scan_only() {
        for expr in [col(SEGMENT_ID).eq(lit("a")), lit("a").eq(col(SEGMENT_ID))] {
            assert_eq!(
                strategy(expr),
                segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                    segment_ids: vec!["a".to_owned()],
                })
            );
        }
        assert_eq!(
            strategy(col(SEGMENT_ID).in_list(vec![lit("a"), lit("b")], false)),
            segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                segment_ids: vec!["a".to_owned(), "b".to_owned()],
            })
        );
    }

    #[test]
    fn segment_id_exclusion_becomes_skip() {
        assert_eq!(
            strategy(col(SEGMENT_ID).not_eq(lit("a"))),
            segment_id_filter::Strategy::Skip(SegmentIdList {
                segment_ids: vec!["a".to_owned()],
            })
        );
        assert_eq!(
            strategy(col(SEGMENT_ID).in_list(vec![lit("a"), lit("b")], true)),
            segment_id_filter::Strategy::Skip(SegmentIdList {
                segment_ids: vec!["a".to_owned(), "b".to_owned()],
            })
        );
    }

    #[test]
    fn segment_id_boolean_expressions_are_normalized() {
        let expr = col(SEGMENT_ID)
            .in_list(vec![lit("a"), lit("b"), lit("c")], false)
            .and(col(SEGMENT_ID).not_eq(lit("b")));
        assert_eq!(
            strategy(expr),
            segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                segment_ids: vec!["a".to_owned(), "c".to_owned()],
            })
        );

        let expr = col(SEGMENT_ID)
            .eq(lit("a"))
            .or(col(SEGMENT_ID).eq(lit("b")));
        assert_eq!(
            strategy(expr),
            segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                segment_ids: vec!["a".to_owned(), "b".to_owned()],
            })
        );
    }

    #[test]
    fn unsupported_segment_id_expressions_stay_client_side() {
        for expr in [
            col("rerun_num_chunks").gt(lit(5i64)),
            col(SEGMENT_ID).gt(lit("a")),
            datafusion::functions::expr_fn::lower(col(SEGMENT_ID)).eq(lit("a")),
            col(SEGMENT_ID).eq(Expr::Literal(ScalarValue::Utf8(None), None)),
        ] {
            assert!(segment_id_filter_from_filters(&[expr], SEGMENT_ID).is_none());
        }
    }

    #[test]
    fn classify_segment_id_filters_as_inexact() {
        let pushable = col(SEGMENT_ID).eq(lit("a"));
        let unsupported = col("rerun_num_chunks").gt(lit(5i64));
        assert_eq!(
            classify_segment_id_filters_for_pushdown(&[&pushable, &unsupported], SEGMENT_ID),
            vec![
                TableProviderFilterPushDown::Inexact,
                TableProviderFilterPushDown::Unsupported,
            ]
        );
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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true, None)
            .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true, None)
            .expect("pushdown succeeds");

        assert!(result.is_none());
    }

    #[test]
    fn test_segment_id_or_creates_multiple_queries() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("rerun_segment_id")
            .eq(lit("segment_a"))
            .or(col("rerun_segment_id").eq(lit("segment_b")));
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true, None)
            .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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

        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true, None)
            .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true, None);

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true, None)
            .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(vec![query], &expr, &schema, true, None)
            .expect("pushdown succeeds");

        assert!(result.is_none());
    }

    // ==================== Error case tests ====================

    #[test]
    fn test_conflicting_segment_ids_error() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_query_with_segment(&SegmentId::from("segment_a"));

        // Try to AND with a different segment
        let expr = col("rerun_segment_id").eq(lit("segment_b"));
        let result = apply_filter_expr_to_queries(vec![query.clone()], &expr, &schema, true, None)
            .expect("pushdown succeeds");

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

    // ==================== Scalar type conversion tests ====================

    #[test]
    fn test_timestamp_nanosecond_conversion() {
        let schema = make_schema_with_index("log_time");
        let query = make_empty_query();

        let expr = col("log_time").eq(Expr::Literal(
            ScalarValue::TimestampNanosecond(Some(1_000_000_000), None),
            None,
        ));
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("log_time")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("log_time")),
        )
        .expect("pushdown succeeds");

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
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("log_time")),
        )
        .expect("pushdown succeeds");

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
        let result = filter_expr_is_supported(
            &expr,
            &query,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("support check succeeds");

        assert_eq!(result, TableProviderFilterPushDown::Inexact);
    }

    #[test]
    fn test_filter_expr_is_supported_returns_unsupported() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("unknown_column").eq(lit(100i64));
        let result = filter_expr_is_supported(
            &expr,
            &query,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("support check succeeds");

        assert_eq!(result, TableProviderFilterPushDown::Unsupported);
    }

    // ==================== Alias expression tests ====================

    #[test]
    fn test_aliased_expression() {
        let schema = make_schema_with_index("frame_nr");
        let query = make_empty_query();

        let expr = col("frame_nr").eq(lit(100i64)).alias("my_filter");
        let result = apply_filter_expr_to_queries(
            vec![query],
            &expr,
            &schema,
            true,
            Some(TimelineName::from("frame_nr")),
        )
        .expect("pushdown succeeds");

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

    // ==================== selection-timeline tests ====================

    /// The reader context of every selection test: reader index `frame_nr`, no fill, no samples.
    fn reader_on_frame_nr() -> SelectionContext {
        SelectionContext {
            reader_index: Some(TimelineName::from("frame_nr")),
            ..Default::default()
        }
    }

    /// The selection timeline and each filter's pushdown, as `supports_filters_pushdown` computes
    /// them.
    fn classify(
        context: &SelectionContext,
        filters: &[&Expr],
        request: &QueryDatasetRequest,
        schema: &SchemaRef,
    ) -> (Option<TimelineName>, Vec<TableProviderFilterPushDown>) {
        let selection_timeline = choose_selection_timeline(context, filters, request, schema)
            .expect("selection succeeds");
        let pushdown = filters
            .iter()
            .map(|filter| {
                filter_expr_is_supported(filter, request, schema, context.fill, selection_timeline)
                    .expect("classification succeeds")
            })
            .collect();
        (selection_timeline, pushdown)
    }

    fn make_schema_with_indexes(index_names: &[&str]) -> SchemaRef {
        let mut metadata = HashMap::new();
        metadata.insert(RERUN_KIND.to_owned(), "index".to_owned());

        let mut fields: Vec<Field> = index_names
            .iter()
            .map(|name| {
                Field::new(*name, arrow::datatypes::DataType::Int64, false)
                    .with_metadata(metadata.clone())
            })
            .collect();
        fields.push(Field::new(
            "rerun_segment_id",
            arrow::datatypes::DataType::Utf8,
            false,
        ));

        Arc::new(Schema::new_with_metadata(fields, HashMap::default()))
    }

    #[test]
    fn u_only_predicate_without_fill_selects_u() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time"]);
        let u_only = col("log_time").gt(lit(100i64));
        let filters = [&u_only];

        let (selection_timeline, pushdown) = classify(
            &reader_on_frame_nr(),
            &filters,
            &make_empty_query(),
            &schema,
        );

        assert_eq!(
            selection_timeline,
            Some(TimelineName::from("log_time")),
            "with no filter or configured bound on T, the first safe foreign timeline wins"
        );
        assert_eq!(pushdown, vec![TableProviderFilterPushDown::Inexact]);
    }

    #[test]
    fn u_only_predicate_with_fill_forces_t_and_stays_residual() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time"]);
        let u_only = col("log_time").gt(lit(100i64));
        let filters = [&u_only];

        let (selection_timeline, pushdown) = classify(
            &SelectionContext {
                fill: true,
                ..reader_on_frame_nr()
            },
            &filters,
            &make_empty_query(),
            &schema,
        );

        assert_eq!(
            selection_timeline,
            Some(TimelineName::from("frame_nr")),
            "latest-at filling requires discovery on the reader index itself"
        );
        assert_eq!(pushdown, vec![TableProviderFilterPushDown::Unsupported]);
    }

    #[test]
    fn t_and_u_conjunction_pushes_t_and_leaves_u_residual() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time"]);
        let expr = col("frame_nr")
            .gt(lit(100i64))
            .and(col("log_time").gt(lit(200i64)));
        let filters = [&expr];

        let (selection_timeline, pushdown) = classify(
            &reader_on_frame_nr(),
            &filters,
            &make_empty_query(),
            &schema,
        );

        assert_eq!(selection_timeline, Some(TimelineName::from("frame_nr")));
        // The whole conjunction is `Inexact`: `T` is pushed, so DataFusion still
        // re-evaluates the full predicate (including the residual `U` half) row-wise.
        assert_eq!(pushdown, vec![TableProviderFilterPushDown::Inexact]);

        let built = apply_filter_expr_to_queries(
            vec![make_empty_query()],
            &expr,
            &schema,
            false,
            selection_timeline,
        )
        .expect("pushdown succeeds")
        .expect("the T half is representable");
        assert_eq!(built.len(), 1);
        let range = built[0]
            .query
            .as_ref()
            .expect("query is set")
            .range
            .as_ref()
            .expect("range is set");
        assert_eq!(
            range.index, "frame_nr",
            "only T's restriction is built into the query"
        );
        assert_eq!(range.index_range.min.as_i64(), 100);
    }

    #[test]
    fn nested_foreign_conjunction_selects_first_usable_timeline() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time", "tick"]);
        let expr = col("log_time")
            .gt(lit(100i64))
            .and(col("tick").lt(lit(200i64)));
        let filters = [&expr];
        let context = reader_on_frame_nr();

        let (selection_timeline, pushdown) =
            classify(&context, &filters, &make_empty_query(), &schema);

        assert_eq!(selection_timeline, Some(TimelineName::from("log_time")));
        assert_eq!(pushdown, vec![TableProviderFilterPushDown::Inexact]);

        let reversed = col("tick")
            .lt(lit(200i64))
            .and(col("log_time").gt(lit(100i64)));
        let reversed_filters = [&reversed];
        let (reversed_selection_timeline, _) =
            classify(&context, &reversed_filters, &make_empty_query(), &schema);
        assert_eq!(
            reversed_selection_timeline,
            Some(TimelineName::from("tick")),
            "expression traversal order decides between safe foreign timelines"
        );
    }

    #[test]
    fn reader_timeline_wins_nested_conjunction_regardless_of_order() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time", "tick"]);
        let context = reader_on_frame_nr();

        for expr in [
            col("log_time")
                .gt(lit(100i64))
                .and(col("frame_nr").lt(lit(200i64))),
            col("frame_nr")
                .lt(lit(200i64))
                .and(col("log_time").gt(lit(100i64))),
        ] {
            let filters = [&expr];
            let (selection_timeline, _) =
                classify(&context, &filters, &make_empty_query(), &schema);
            assert_eq!(selection_timeline, Some(TimelineName::from("frame_nr")));
        }
    }

    #[test]
    fn usable_foreign_selection_survives_unsupported_filter_omission() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time", "tick", "other_time"]);
        let unsafe_or = col("log_time")
            .gt(lit(100i64))
            .or(col("tick").lt(lit(200i64)));
        let segment = col("rerun_segment_id").eq(lit("segment_a"));
        let safe_w = col("other_time").lt(lit(300i64));
        let planning_filters = [&unsafe_or, &segment, &safe_w];
        let context = reader_on_frame_nr();

        let (planned_selection_timeline, planned_pushdown) =
            classify(&context, &planning_filters, &make_empty_query(), &schema);
        assert_eq!(
            planned_selection_timeline,
            Some(TimelineName::from("other_time")),
            "a segment predicate cannot witness either unsafe OR timeline"
        );
        assert_eq!(
            planned_pushdown,
            vec![
                TableProviderFilterPushDown::Unsupported,
                TableProviderFilterPushDown::Inexact,
                TableProviderFilterPushDown::Inexact,
            ]
        );

        let scan_filters = [&segment, &safe_w];
        let (scan_selection_timeline, scan_pushdown) =
            classify(&context, &scan_filters, &make_empty_query(), &schema);
        assert_eq!(scan_selection_timeline, planned_selection_timeline);
        assert_eq!(
            scan_pushdown,
            vec![
                TableProviderFilterPushDown::Inexact,
                TableProviderFilterPushDown::Inexact,
            ]
        );
    }

    /// A filter that compares the reader index but cannot be pushed as a whole does not restrict
    /// the reader: DataFusion removes it before `scan`.
    #[test]
    fn unpushable_reader_index_filter_does_not_restrict_reader() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time"]);
        // The `frame_nr` branch is pushable, but `foo` is not an index, so the OR is not.
        let unpushable_t = col("frame_nr").gt(lit(5i64)).or(col("foo").gt(lit(3i64)));
        let u_and_segment = col("log_time")
            .gt(lit(100i64))
            .and(col("rerun_segment_id").eq(lit("x")));
        let context = reader_on_frame_nr();

        let planning_filters = [&unpushable_t, &u_and_segment];
        let (planned_selection_timeline, planned_pushdown) =
            classify(&context, &planning_filters, &make_empty_query(), &schema);
        assert_eq!(
            planned_selection_timeline,
            Some(TimelineName::from("log_time")),
            "an unpushable comparison on the reader index must not pin selection to it"
        );
        assert_eq!(
            planned_pushdown,
            vec![
                TableProviderFilterPushDown::Unsupported,
                TableProviderFilterPushDown::Inexact,
            ]
        );

        let scan_filters = [&u_and_segment];
        let (scan_selection_timeline, scan_pushdown) =
            classify(&context, &scan_filters, &make_empty_query(), &schema);
        assert_eq!(
            scan_selection_timeline, planned_selection_timeline,
            "planning and scanning must choose the same selection timeline"
        );
        assert_eq!(scan_pushdown, vec![TableProviderFilterPushDown::Inexact]);
    }

    #[test]
    fn configured_reader_range_counts_as_t_restriction() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time"]);
        let u_only = col("log_time").gt(lit(100i64));
        let filters = [&u_only];

        // No filter names `frame_nr`, but the base request already carries a
        // `T` range (e.g. from a configured `filtered_index_range`).
        let request = QueryDatasetRequest {
            query: Some(Query {
                range: Some(QueryRange {
                    index: TimelineName::from("frame_nr"),
                    index_range: AbsoluteTimeRange {
                        min: TimeInt::new_temporal(0),
                        max: TimeInt::new_temporal(10),
                    },
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (selection_timeline, pushdown) =
            classify(&reader_on_frame_nr(), &filters, &request, &schema);

        assert_eq!(selection_timeline, Some(TimelineName::from("frame_nr")));
        assert_eq!(pushdown, vec![TableProviderFilterPushDown::Unsupported]);
    }

    #[test]
    fn mixed_timeline_or_is_residual() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time"]);
        let expr = col("frame_nr")
            .gt(lit(100i64))
            .or(col("log_time").gt(lit(200i64)));
        let filters = [&expr];

        let (selection_timeline, pushdown) = classify(
            &reader_on_frame_nr(),
            &filters,
            &make_empty_query(),
            &schema,
        );

        assert_eq!(selection_timeline, Some(TimelineName::from("frame_nr")));
        assert_eq!(
            pushdown,
            vec![TableProviderFilterPushDown::Unsupported],
            "an OR whose branches span two timelines must never intersect their \
             candidate sets; it stays residual and the scan uses the broader request"
        );
    }

    #[test]
    fn same_timeline_or_still_fans_out() {
        let schema = make_schema_with_indexes(&["frame_nr", "log_time"]);
        let expr = col("frame_nr")
            .gt(lit(100i64))
            .or(col("frame_nr").lt(lit(10i64)));
        let filters = [&expr];

        let (selection_timeline, pushdown) = classify(
            &reader_on_frame_nr(),
            &filters,
            &make_empty_query(),
            &schema,
        );

        assert_eq!(selection_timeline, Some(TimelineName::from("frame_nr")));
        assert_eq!(pushdown, vec![TableProviderFilterPushDown::Inexact]);

        let built = apply_filter_expr_to_queries(
            vec![make_empty_query()],
            &expr,
            &schema,
            false,
            selection_timeline,
        )
        .expect("pushdown succeeds")
        .expect("a same-timeline OR still fans out into one query per branch");
        assert_eq!(built.len(), 2);
    }

    #[test]
    fn static_reader_keeps_every_index_predicate_residual() {
        let schema = make_schema_with_indexes(&["frame_nr"]);
        let expr = col("frame_nr").gt(lit(100i64));
        let filters = [&expr];

        let (selection_timeline, pushdown) = classify(
            &SelectionContext::default(),
            &filters,
            &make_empty_query(),
            &schema,
        );

        assert_eq!(selection_timeline, None);
        assert_eq!(pushdown, vec![TableProviderFilterPushDown::Unsupported]);
    }
}
