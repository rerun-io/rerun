//! Released SDKs send `QueryDataset`, and they are answered through `FindChunks` with the code
//! in this module. New SDKs send `FindChunks` directly and never reach it.
//!
//! TODO(RR-5817): remove once the SDKs that send `QueryDataset` are unsupported.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch};
use arrow::datatypes::{FieldRef, Schema};
use re_chunk::TimelineName;
use re_log_types::{EntityPath, TimeInt};

use crate::cloud::v1alpha1::QueryDatasetResponse;
use crate::cloud::v1alpha1::ext::{
    ChunkCandidatesDataframe, ChunkMode, ComponentSelection, ContentSelection, DirectUrlMode,
    EntitySelection, FillMode, FindChunksRequest, Query, QueryDatasetDataframe,
    QueryDatasetRequest, RangeSelection, SegmentSelection,
};

/// The result of mapping a legacy [`QueryDatasetRequest`] onto the new selection model.
///
/// See [`adapt_legacy`] for the mapping rules.
///
/// TODO(RR-5817): remove with the rest of the `QueryDataset` compatibility code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyPlan {
    /// The legacy request has an exact [`FindChunksRequest`] equivalent.
    Find(FindChunksRequest),

    /// The legacy request has no time filter: neither a `latest_at` nor a `range`.
    ///
    /// Released SDKs send this for a plain `reader(index=…)`.
    /// They leave the reader's timeline out of the request (although they should have),
    /// so no `FindChunks` mode fits.
    /// The server answers with every chunk of the selected segments and contents,
    /// static or temporal, as `QueryDataset` did.
    FullScan {
        segments: SegmentSelection,
        contents: ContentSelection,
        direct_url_mode: DirectUrlMode,
    },
}

/// Maps a legacy [`QueryDatasetRequest`] onto the new selection model, or rejects it when no
/// exact, candidate-preserving mapping exists.
///
/// Rejects the fields the new selection model cannot express before choosing a mode, so an
/// otherwise-empty selection never hides a rejected combination.
///
/// `request` must come from the ext `TryFrom<proto::QueryDatasetRequest>` conversion, which
/// enforces the `per_segment_values` invariants. A violation found here returns
/// `Status::internal`.
///
/// TODO(RR-5817): remove with the rest of the `QueryDataset` compatibility code.
pub fn adapt_legacy(request: QueryDatasetRequest) -> Result<LegacyPlan, tonic::Status> {
    let QueryDatasetRequest {
        segment_ids,
        generate_direct_urls,
        unsigned_direct_urls,
        chunk_ids,
        entity_paths,
        select_all_entity_paths,
        fuzzy_descriptors,
        exclude_static_data,
        exclude_temporal_data,
        scan_parameters,
        query,
    } = request;

    if select_all_entity_paths && !entity_paths.is_empty() {
        return Err(tonic::Status::invalid_argument(
            "`select_all_entity_paths` is true, but `entity_paths` is not empty",
        ));
    }
    if !chunk_ids.is_empty() {
        return Err(tonic::Status::invalid_argument(
            "`chunk_ids` has no equivalent in `FindChunks`",
        ));
    }
    if exclude_static_data {
        return Err(tonic::Status::invalid_argument(
            "`exclude_static_data` has no equivalent in `FindChunks`",
        ));
    }
    if exclude_temporal_data {
        return Err(tonic::Status::invalid_argument(
            "`exclude_temporal_data` has no equivalent in `FindChunks`",
        ));
    }
    if let Some(scan_parameters) = &scan_parameters {
        if scan_parameters.filter.is_some() {
            return Err(tonic::Status::invalid_argument(
                "`scan_parameters.filter` has no equivalent in `FindChunks`",
            ));
        }
        if scan_parameters.limit_offset.is_some() {
            return Err(tonic::Status::invalid_argument(
                "`scan_parameters.limit_offset` has no equivalent in `FindChunks`",
            ));
        }
        if scan_parameters.limit_len.is_some() {
            return Err(tonic::Status::invalid_argument(
                "`scan_parameters.limit_len` has no equivalent in `FindChunks`",
            ));
        }
        if !scan_parameters.order_by.is_empty() {
            return Err(tonic::Status::invalid_argument(
                "`scan_parameters.order_by` has no equivalent in `FindChunks`",
            ));
        }
    }

    if let Some(query) = &query
        && let Some(latest_at) = &query.latest_at
        && !latest_at.per_segment_values.is_empty()
    {
        let mut seen = HashSet::with_capacity(segment_ids.len());
        let has_duplicate_segment_id =
            !segment_ids.iter().all(|segment_id| seen.insert(segment_id));

        let invariant_violated = latest_at.index.is_none()
            || latest_at.at != TimeInt::STATIC
            || query.range.is_some()
            || segment_ids.is_empty()
            || latest_at.per_segment_values.len() != segment_ids.len()
            || has_duplicate_segment_id;

        if invariant_violated {
            return Err(tonic::Status::internal(
                "`per_segment_values` violates the invariants the ext `TryFrom` already enforces",
            ));
        }
    }

    // A latest-at that is not anchored at the range start has no exact mapping onto
    // `FillMode::LatestAt`.
    if let Some(query) = &query
        && let Some(range) = &query.range
        && let Some(latest_at) = &query.latest_at
        && !latest_at.is_static()
        && (latest_at.at != range.index_range.min || latest_at.index != Some(range.index))
    {
        return Err(tonic::Status::invalid_argument(
            "`latest_at` must be anchored at `range.min` on the same timeline to map onto \
             `FILL_MODE_LATEST_AT`",
        ));
    }

    let segments = if segment_ids.is_empty() {
        SegmentSelection::All
    } else {
        SegmentSelection::Explicit(segment_ids.iter().cloned().collect())
    };

    // `ContentSelection::All` cannot carry components, so `fuzzy_descriptors` is dropped here.
    // A caller that must keep them lists the entities first (RR-5816).
    let contents = if select_all_entity_paths {
        ContentSelection::All
    } else {
        let components = if fuzzy_descriptors.is_empty() {
            ComponentSelection::All
        } else {
            ComponentSelection::Explicit(fuzzy_descriptors.into_iter().collect())
        };
        ContentSelection::Explicit(
            entity_paths
                .into_iter()
                .map(|entity_path| EntitySelection {
                    entity_path,
                    components: components.clone(),
                })
                .collect(),
        )
    };

    let direct_url_mode = match (generate_direct_urls, unsigned_direct_urls) {
        (false, _) => DirectUrlMode::Omit,
        (true, true) => DirectUrlMode::Unsigned,
        (true, false) => DirectUrlMode::Signed,
    };

    let Some(Query {
        latest_at, range, ..
    }) = query
    else {
        return Ok(LegacyPlan::FullScan {
            segments,
            contents,
            direct_url_mode,
        });
    };

    match (latest_at, range) {
        (Some(latest_at), _) if !latest_at.per_segment_values.is_empty() => {
            let Some(timeline) = latest_at.index else {
                return Err(tonic::Status::internal(
                    "`per_segment_values` set without `latest_at.index`",
                ));
            };

            let mut plan_segments = BTreeMap::new();
            for (segment_id, values) in
                std::iter::zip(segment_ids.iter(), latest_at.per_segment_values.iter())
            {
                if values.is_empty() {
                    continue;
                }
                plan_segments.insert(
                    segment_id.clone(),
                    values
                        .iter()
                        .map(|&value| TimeInt::new_temporal(value))
                        .collect(),
                );
            }

            Ok(LegacyPlan::Find(FindChunksRequest {
                contents,
                mode: ChunkMode::Samples {
                    timeline,
                    segments: plan_segments,
                    fill_mode: FillMode::LatestAt,
                },
                direct_url_mode,
            }))
        }

        (Some(latest_at), None) if latest_at.is_static() => {
            Ok(LegacyPlan::Find(FindChunksRequest {
                contents,
                mode: ChunkMode::Static { segments },
                direct_url_mode,
            }))
        }

        (latest_at, Some(range)) => {
            let fill_mode = if latest_at.is_some_and(|latest_at| !latest_at.is_static()) {
                FillMode::LatestAt
            } else {
                FillMode::None
            };
            Ok(LegacyPlan::Find(FindChunksRequest {
                contents,
                mode: ChunkMode::Range {
                    segments,
                    range: RangeSelection {
                        timeline: range.index,
                        start_inclusive: Some(range.index_range.min),
                        end_inclusive: Some(range.index_range.max),
                        fill_mode,
                    },
                },
                direct_url_mode,
            }))
        }

        (Some(latest_at), None) => {
            let Some(timeline) = latest_at.index else {
                return Err(tonic::Status::internal(
                    "temporal `latest_at` reached with no `index`",
                ));
            };
            if latest_at.at == TimeInt::STATIC {
                return Err(tonic::Status::invalid_argument(
                    "`latest_at.at` must be set when `latest_at.index` names a temporal timeline",
                ));
            }

            Ok(LegacyPlan::Find(FindChunksRequest {
                contents,
                mode: ChunkMode::Range {
                    segments,
                    range: RangeSelection {
                        timeline,
                        start_inclusive: Some(latest_at.at),
                        end_inclusive: Some(latest_at.at),
                        fill_mode: FillMode::LatestAt,
                    },
                },
                direct_url_mode,
            }))
        }

        (None, None) => Ok(LegacyPlan::FullScan {
            segments,
            contents,
            direct_url_mode,
        }),
    }
}

/// Whether `request` selects components on every entity: `select_all_entity_paths` together
/// with `fuzzy_descriptors`.
///
/// [`adapt_legacy`] cannot express this, so the server first calls
/// [`list_entities_for_component_selection`].
///
/// TODO(RR-5817): remove with the rest of the `QueryDataset` compatibility code.
pub fn selects_components_on_every_entity(request: &QueryDatasetRequest) -> bool {
    request.select_all_entity_paths && !request.fuzzy_descriptors.is_empty()
}

/// Rewrites a request that selects components on every entity so that it lists the entities of
/// `dataset_schema` instead, and leaves any other request unchanged.
///
/// Compatibility code for released SDKs: new SDKs send `FindChunks` and never need it.
/// Those SDKs send `select_all_entity_paths` with `fuzzy_descriptors` for a projected
/// `reader()` (RR-5816). `FindChunks` ties components to named entities, so [`adapt_legacy`]
/// would drop the components, and the read would fetch every chunk.
///
/// TODO(RR-5817): remove with the rest of the `QueryDataset` compatibility code.
pub fn list_entities_for_component_selection(
    request: QueryDatasetRequest,
    dataset_schema: &Schema,
) -> QueryDatasetRequest {
    if !selects_components_on_every_entity(&request) {
        return request;
    }

    let entity_paths = dataset_schema
        .fields()
        .iter()
        .filter_map(|field| {
            field
                .metadata()
                .get(re_sorbet::metadata::SORBET_ENTITY_PATH)
        })
        .map(|entity_path| EntityPath::parse_forgiving(entity_path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    QueryDatasetRequest {
        select_all_entity_paths: false,
        entity_paths,
        ..request
    }
}

/// Converts a [`ChunkCandidatesDataframe`] batch into the legacy [`QueryDatasetDataframe`] shape.
///
/// Preserves the ten base columns in [`QueryDatasetDataframe::min_schema`] order, reusing their
/// Arrow arrays unchanged. When `timeline` is known, appends the existing `chunk_timeline_start`
/// array as `{timeline}:start` (see [`QueryDatasetResponse::field_timeline_start`]). Without a
/// timeline, the response carries no start column.
///
/// TODO(RR-5817): remove with the rest of the `QueryDataset` compatibility code.
pub fn encode_legacy_candidates(
    batch: RecordBatch,
    timeline: Option<&TimelineName>,
) -> Result<RecordBatch, tonic::Status> {
    let base_schema = QueryDatasetDataframe::min_schema();

    let mut fields: Vec<FieldRef> = Vec::with_capacity(base_schema.fields().len() + 1);
    let mut columns: Vec<ArrayRef> = Vec::with_capacity(base_schema.fields().len() + 1);

    for field in base_schema.fields() {
        let array = batch.column_by_name(field.name()).ok_or_else(|| {
            tonic::Status::internal(format!(
                "candidate batch is missing column `{}`",
                field.name()
            ))
        })?;
        fields.push(Arc::clone(field));
        columns.push(Arc::clone(array));
    }

    if let Some(timeline) = timeline {
        let start_array = batch
            .column_by_name(ChunkCandidatesDataframe::COLUMN_CHUNK_TIMELINE_START_NAME)
            .ok_or_else(|| {
                tonic::Status::internal("candidate batch is missing column `chunk_timeline_start`")
            })?;
        fields.push(QueryDatasetResponse::field_timeline_start(
            timeline.as_str(),
        ));
        columns.push(Arc::clone(start_array));
    }

    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).map_err(|err| {
        tonic::Status::internal(format!("failed to build legacy candidate batch: {err}"))
    })
}

/// Builds the `QueryDatasetRequest` that answers a [`LegacyPlan::FullScan`]: no time filter
/// (`query: Some(Query::default())`), over the given segments and contents.
///
/// The Hub and the OSS server both use it, so they answer a full scan with the same chunks.
///
/// TODO(RR-5817): remove with the rest of the `QueryDataset` compatibility code.
pub fn full_scan_query_dataset_request(
    segments: SegmentSelection,
    contents: ContentSelection,
    direct_url_mode: DirectUrlMode,
) -> QueryDatasetRequest {
    let segment_ids = match segments {
        SegmentSelection::All => vec![],
        SegmentSelection::Explicit(ids) => ids.into_iter().collect(),
    };

    let (select_all_entity_paths, entity_paths, fuzzy_descriptors) = match contents {
        ContentSelection::All => (true, vec![], vec![]),
        ContentSelection::Explicit(entities) => {
            let entity_paths = entities
                .iter()
                .map(|entity| entity.entity_path.clone())
                .collect();

            let fuzzy_descriptors = if entities
                .iter()
                .any(|entity| matches!(entity.components, ComponentSelection::All))
            {
                vec![]
            } else {
                entities
                    .iter()
                    .filter_map(|entity| match &entity.components {
                        ComponentSelection::Explicit(ids) => Some(ids.iter().cloned()),
                        ComponentSelection::All => None,
                    })
                    .flatten()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            };

            (false, entity_paths, fuzzy_descriptors)
        }
    };

    QueryDatasetRequest {
        segment_ids,
        generate_direct_urls: direct_url_mode != DirectUrlMode::Omit,
        unsigned_direct_urls: direct_url_mode == DirectUrlMode::Unsigned,
        entity_paths,
        select_all_entity_paths,
        fuzzy_descriptors,
        query: Some(Query::default()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use arrow::array::{Array as _, Int64Array};
    use arrow::datatypes::{DataType, Field};
    use re_log_types::AbsoluteTimeRange;
    use re_types_core::LayerName;

    use crate::cloud::v1alpha1::ext::{QueryLatestAt, QueryRange};
    use crate::common::v1alpha1::ext as common_ext;
    use crate::common::v1alpha1::ext::SegmentId;

    use super::*;

    /// Every mapping and rejection rule of [`adapt_legacy`].
    ///
    /// Several SDK wire shapes map onto the same `QueryDatasetRequest`; each case names the SDK
    /// calls it stands for. The ext `TryFrom` tests own the `per_segment_values` invariants.
    #[test]
    fn adapt_legacy_mapping() {
        fn request(mutate: impl FnOnce(&mut QueryDatasetRequest)) -> QueryDatasetRequest {
            let mut request = QueryDatasetRequest::default();
            mutate(&mut request);
            request
        }

        fn temporal_range(timeline: &'static str, min: TimeInt, max: TimeInt) -> QueryRange {
            QueryRange {
                index: TimelineName::from(timeline),
                index_range: AbsoluteTimeRange { min, max },
            }
        }

        fn all_static_plan(direct_url_mode: DirectUrlMode) -> LegacyPlan {
            LegacyPlan::Find(FindChunksRequest {
                contents: ContentSelection::All,
                mode: ChunkMode::Static {
                    segments: SegmentSelection::All,
                },
                direct_url_mode,
            })
        }

        fn range_plan(
            timeline: &'static str,
            start_inclusive: Option<TimeInt>,
            end_inclusive: Option<TimeInt>,
            fill_mode: FillMode,
            omit_direct_urls: bool,
        ) -> LegacyPlan {
            LegacyPlan::Find(FindChunksRequest {
                contents: ContentSelection::All,
                mode: ChunkMode::Range {
                    segments: SegmentSelection::All,
                    range: RangeSelection {
                        timeline: TimelineName::from(timeline),
                        start_inclusive,
                        end_inclusive,
                        fill_mode,
                    },
                },
                direct_url_mode: if omit_direct_urls {
                    DirectUrlMode::Omit
                } else {
                    DirectUrlMode::Signed
                },
            })
        }

        enum Expect {
            Plan(LegacyPlan),
            Err(&'static str),
        }

        struct Case {
            name: &'static str,
            request: QueryDatasetRequest,
            expect: Expect,
        }

        let cases = vec![
            Case {
                name: "reader(index=None): `latest_at { index: None }` (`is_static()`) -> Static",
                request: request(|r| {
                    r.query = Some(Query {
                        latest_at: Some(QueryLatestAt::new_static()),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(all_static_plan(DirectUrlMode::Omit)),
            },
            Case {
                name: "reader(index=T) no filter, and Rust `index_values` without segment ids: \
                       `Query{None,None}` carries no timeline -> FullScan",
                request: request(|r| {
                    r.query = Some(Query::default());
                }),
                expect: Expect::Plan(LegacyPlan::FullScan {
                    segments: SegmentSelection::All,
                    contents: ContentSelection::All,
                    direct_url_mode: DirectUrlMode::Omit,
                }),
            },
            Case {
                name: "`query: None` -> FullScan",
                request: request(|_| {}),
                expect: Expect::Plan(LegacyPlan::FullScan {
                    segments: SegmentSelection::All,
                    contents: ContentSelection::All,
                    direct_url_mode: DirectUrlMode::Omit,
                }),
            },
            Case {
                name: "filter `T = t`, fill off -> Range{T,t,t,false} (equal-bound Range, fill off)",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range(
                            "frame",
                            TimeInt::new_temporal(10),
                            TimeInt::new_temporal(10),
                        )),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::new_temporal(10)),
                    Some(TimeInt::new_temporal(10)),
                    FillMode::None,
                    true,
                )),
            },
            Case {
                name: "filter `T = t`, fill on -> Range{T,t,t,true} (equal-bound Range, fill on)",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range(
                            "frame",
                            TimeInt::new_temporal(10),
                            TimeInt::new_temporal(10),
                        )),
                        latest_at: Some(QueryLatestAt::global(
                            Some(TimelineName::from("frame")),
                            TimeInt::new_temporal(10),
                        )),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::new_temporal(10)),
                    Some(TimeInt::new_temporal(10)),
                    FillMode::LatestAt,
                    true,
                )),
            },
            Case {
                name: "filter `T > t` or `T >= t`, fill on -> Range{T,t,MAX,true}",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range(
                            "frame",
                            TimeInt::new_temporal(5),
                            TimeInt::MAX,
                        )),
                        latest_at: Some(QueryLatestAt::global(
                            Some(TimelineName::from("frame")),
                            TimeInt::new_temporal(5),
                        )),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::new_temporal(5)),
                    Some(TimeInt::MAX),
                    FillMode::LatestAt,
                    true,
                )),
            },
            Case {
                name: "filter `T < t`/`T <= t` after the legacy AND merge with fill: \
                       `range[MIN,t]` + `latest_at(MIN)` -> Range{T,MIN,t,true}",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range(
                            "frame",
                            TimeInt::MIN,
                            TimeInt::new_temporal(5),
                        )),
                        latest_at: Some(QueryLatestAt::global(
                            Some(TimelineName::from("frame")),
                            TimeInt::MIN,
                        )),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::MIN),
                    Some(TimeInt::new_temporal(5)),
                    FillMode::LatestAt,
                    true,
                )),
            },
            Case {
                name: "`T BETWEEN a AND b`, fill off, with explicit MIN/MAX endpoints preserved \
                       rather than normalized -> Range{T,MIN,MAX,false}",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range("frame", TimeInt::MIN, TimeInt::MAX)),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::MIN),
                    Some(TimeInt::MAX),
                    FillMode::None,
                    true,
                )),
            },
            Case {
                name: "the viewer's manifest-less Range with fill: `latest_at_range` pairs `at` \
                       with `range.min`",
                request: request(|r| {
                    r.query = Some(Query::latest_at_range(
                        TimelineName::from("frame"),
                        AbsoluteTimeRange {
                            min: TimeInt::new_temporal(1),
                            max: TimeInt::new_temporal(9),
                        },
                    ));
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::new_temporal(1)),
                    Some(TimeInt::new_temporal(9)),
                    FillMode::LatestAt,
                    true,
                )),
            },
            Case {
                name: "a static latest-at paired with a range clears fill",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range(
                            "frame",
                            TimeInt::new_temporal(1),
                            TimeInt::new_temporal(9),
                        )),
                        latest_at: Some(QueryLatestAt::new_static()),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::new_temporal(1)),
                    Some(TimeInt::new_temporal(9)),
                    FillMode::None,
                    true,
                )),
            },
            Case {
                name: "standalone temporal latest-at without a range -> equal-bound Range with fill",
                request: request(|r| {
                    r.query = Some(Query {
                        latest_at: Some(QueryLatestAt::global(
                            Some(TimelineName::from("frame")),
                            TimeInt::new_temporal(7),
                        )),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(range_plan(
                    "frame",
                    Some(TimeInt::new_temporal(7)),
                    Some(TimeInt::new_temporal(7)),
                    FillMode::LatestAt,
                    true,
                )),
            },
            Case {
                name: "a temporal index with the static sentinel as its value is rejected",
                request: request(|r| {
                    r.query = Some(Query {
                        latest_at: Some(QueryLatestAt {
                            index: Some(TimelineName::from("frame")),
                            at: TimeInt::STATIC,
                            per_segment_values: vec![],
                        }),
                        ..Query::default()
                    });
                }),
                expect: Expect::Err("temporal"),
            },
            Case {
                name: "mismatched time: `latest_at.at` != `range.min` on the same timeline",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range(
                            "frame",
                            TimeInt::new_temporal(1),
                            TimeInt::new_temporal(9),
                        )),
                        latest_at: Some(QueryLatestAt::global(
                            Some(TimelineName::from("frame")),
                            TimeInt::new_temporal(2),
                        )),
                        ..Query::default()
                    });
                }),
                expect: Expect::Err("anchored"),
            },
            Case {
                name: "mismatched timeline: `latest_at` names a different index than `range`",
                request: request(|r| {
                    r.query = Some(Query {
                        range: Some(temporal_range(
                            "frame",
                            TimeInt::new_temporal(1),
                            TimeInt::new_temporal(9),
                        )),
                        latest_at: Some(QueryLatestAt::global(
                            Some(TimelineName::from("other")),
                            TimeInt::new_temporal(1),
                        )),
                        ..Query::default()
                    });
                }),
                expect: Expect::Err("anchored"),
            },
            Case {
                name: "`using_index_values={seg: values}` -> Samples, empty-value segments dropped",
                request: request(|r| {
                    r.segment_ids = vec![SegmentId::from("seg1"), SegmentId::from("seg2")];
                    r.query = Some(Query {
                        latest_at: Some(QueryLatestAt {
                            index: Some(TimelineName::from("frame")),
                            at: TimeInt::STATIC,
                            per_segment_values: vec![vec![1, 2], vec![]],
                        }),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(LegacyPlan::Find(FindChunksRequest {
                    contents: ContentSelection::All,
                    mode: ChunkMode::Samples {
                        timeline: TimelineName::from("frame"),
                        segments: BTreeMap::from([(
                            SegmentId::from("seg1"),
                            BTreeSet::from([TimeInt::new_temporal(1), TimeInt::new_temporal(2)]),
                        )]),
                        fill_mode: FillMode::LatestAt,
                    },
                    direct_url_mode: DirectUrlMode::Omit,
                })),
            },
            Case {
                name: "`filter_segments(ids)`: non-empty `segment_ids` -> Explicit(ids), deduplicated",
                request: request(|r| {
                    r.segment_ids = vec![SegmentId::from("seg1"), SegmentId::from("seg1")];
                    r.query = Some(Query {
                        latest_at: Some(QueryLatestAt::new_static()),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(LegacyPlan::Find(FindChunksRequest {
                    contents: ContentSelection::All,
                    mode: ChunkMode::Static {
                        segments: SegmentSelection::Explicit(BTreeSet::from([SegmentId::from(
                            "seg1",
                        )])),
                    },
                    direct_url_mode: DirectUrlMode::Omit,
                })),
            },
            Case {
                name: "projection/filter narrows components: flat `fuzzy_descriptors` apply to \
                       every entity",
                request: request(|r| {
                    r.select_all_entity_paths = false;
                    r.entity_paths = vec![
                        EntityPath::parse_strict("a").expect("valid entity path literal"),
                        EntityPath::parse_strict("b").expect("valid entity path literal"),
                    ];
                    r.fuzzy_descriptors = vec!["Points3D:positions".to_owned()];
                    r.query = Some(Query {
                        latest_at: Some(QueryLatestAt::new_static()),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(LegacyPlan::Find(FindChunksRequest {
                    contents: ContentSelection::Explicit(vec![
                        EntitySelection {
                            entity_path: EntityPath::parse_strict("a")
                                .expect("valid entity path literal"),
                            components: ComponentSelection::Explicit(BTreeSet::from([
                                "Points3D:positions".to_owned(),
                            ])),
                        },
                        EntitySelection {
                            entity_path: EntityPath::parse_strict("b")
                                .expect("valid entity path literal"),
                            components: ComponentSelection::Explicit(BTreeSet::from([
                                "Points3D:positions".to_owned(),
                            ])),
                        },
                    ]),
                    mode: ChunkMode::Static {
                        segments: SegmentSelection::All,
                    },
                    direct_url_mode: DirectUrlMode::Omit,
                })),
            },
            Case {
                name: "signing: `generate_direct_urls=true` -> signed direct URLs",
                request: request(|r| {
                    r.generate_direct_urls = true;
                    r.query = Some(Query {
                        latest_at: Some(QueryLatestAt::new_static()),
                        ..Query::default()
                    });
                }),
                expect: Expect::Plan(all_static_plan(DirectUrlMode::Signed)),
            },
            Case {
                name: "`select_all_entity_paths` with non-empty `entity_paths` is rejected",
                request: request(|r| {
                    r.select_all_entity_paths = true;
                    r.entity_paths =
                        vec![EntityPath::parse_strict("a").expect("valid entity path literal")];
                }),
                expect: Expect::Err("select_all_entity_paths"),
            },
            Case {
                name: "`chunk_ids` is rejected",
                request: request(|r| {
                    r.chunk_ids = vec![re_chunk::ChunkId::new()];
                }),
                expect: Expect::Err("chunk_ids"),
            },
            Case {
                name: "`exclude_static_data` is rejected",
                request: request(|r| {
                    r.exclude_static_data = true;
                }),
                expect: Expect::Err("exclude_static_data"),
            },
            Case {
                name: "`exclude_temporal_data` is rejected",
                request: request(|r| {
                    r.exclude_temporal_data = true;
                }),
                expect: Expect::Err("exclude_temporal_data"),
            },
            Case {
                name: "`scan_parameters.filter` is rejected",
                request: request(|r| {
                    r.scan_parameters = Some(common_ext::ScanParameters {
                        filter: Some("true".to_owned()),
                        ..Default::default()
                    });
                }),
                expect: Expect::Err("scan_parameters.filter"),
            },
            Case {
                name: "`scan_parameters.limit_offset` is rejected",
                request: request(|r| {
                    r.scan_parameters = Some(common_ext::ScanParameters {
                        limit_offset: Some(1),
                        ..Default::default()
                    });
                }),
                expect: Expect::Err("scan_parameters.limit_offset"),
            },
            Case {
                name: "`scan_parameters.limit_len` is rejected",
                request: request(|r| {
                    r.scan_parameters = Some(common_ext::ScanParameters {
                        limit_len: Some(1),
                        ..Default::default()
                    });
                }),
                expect: Expect::Err("scan_parameters.limit_len"),
            },
            Case {
                name: "`scan_parameters.order_by` is rejected",
                request: request(|r| {
                    r.scan_parameters = Some(common_ext::ScanParameters {
                        order_by: vec![common_ext::ScanParametersOrderClause {
                            descending: false,
                            nulls_last: false,
                            column_name: "chunk_id".to_owned(),
                        }],
                        ..Default::default()
                    });
                }),
                expect: Expect::Err("scan_parameters.order_by"),
            },
            Case {
                name: "an invalid combination is rejected even inside an otherwise-empty selection",
                request: request(|r| {
                    r.select_all_entity_paths = false;
                    r.entity_paths = vec![];
                    r.chunk_ids = vec![re_chunk::ChunkId::new()];
                }),
                expect: Expect::Err("chunk_ids"),
            },
        ];

        for case in cases {
            let result = adapt_legacy(case.request);
            match case.expect {
                Expect::Plan(expected) => {
                    let actual = result
                        .unwrap_or_else(|err| panic!("{}: expected Ok, got {err}", case.name));
                    assert_eq!(actual, expected, "{}", case.name);
                }
                Expect::Err(substr) => {
                    let err = result
                        .err()
                        .unwrap_or_else(|| panic!("{}: expected an error", case.name));
                    assert!(
                        err.message().contains(substr),
                        "{}: expected message to contain {substr:?}, got {}",
                        case.name,
                        err.message()
                    );
                }
            }
        }
    }

    /// `encode_legacy_candidates`: column order, legacy field metadata, null preservation, and
    /// unchanged base-column values, for populated and empty batches, with and without a known
    /// timeline.
    #[test]
    fn list_entities_for_component_selection_lists_schema_entities() {
        fn entity_field(name: &str, entity_path: &str) -> Field {
            Field::new(name, DataType::Int64, true).with_metadata(
                [(
                    re_sorbet::metadata::SORBET_ENTITY_PATH.to_owned(),
                    entity_path.to_owned(),
                )]
                .into(),
            )
        }

        let schema = Schema::new(vec![
            Field::new("frame", DataType::Int64, true),
            entity_field("/camera:Image:buffer", "/camera"),
            entity_field("/camera:Image:format", "/camera"),
            entity_field("/lidar:Points3D:positions", "/lidar"),
        ]);

        let request = QueryDatasetRequest {
            select_all_entity_paths: true,
            fuzzy_descriptors: vec!["Image:buffer".to_owned()],
            ..Default::default()
        };
        let rewritten = list_entities_for_component_selection(request, &schema);
        assert!(!rewritten.select_all_entity_paths);
        assert_eq!(
            rewritten.entity_paths,
            vec![EntityPath::from("/camera"), EntityPath::from("/lidar")]
        );
        assert_eq!(rewritten.fuzzy_descriptors, vec!["Image:buffer".to_owned()]);

        let without_components = QueryDatasetRequest {
            select_all_entity_paths: true,
            ..Default::default()
        };
        let unchanged = list_entities_for_component_selection(without_components, &schema);
        assert!(unchanged.select_all_entity_paths);
        assert!(unchanged.entity_paths.is_empty());
    }

    #[test]
    fn encode_legacy_candidates_columns() {
        fn populated_batch() -> RecordBatch {
            ChunkCandidatesDataframe::new(
                vec![re_chunk::ChunkId::new(), re_chunk::ChunkId::new()],
                vec![SegmentId::from("seg1"), SegmentId::from("seg2")],
                vec![LayerName::from("layer"), LayerName::from("layer")],
                vec![b"key1".as_slice(), b"key2".as_slice()],
                vec![EntityPath::root(), EntityPath::root()],
                vec![true, false],
                vec![1u64, 2u64],
                vec![Some(10u64), None],
                vec![Some(5i64), None],
                vec![Some("https://x/1".to_owned()), None],
                vec![Some(1_000i64), None],
            )
            .expect("valid candidate columns")
            .into_record_batch()
            .expect("valid candidate record batch")
        }

        struct Case {
            name: &'static str,
            batch: RecordBatch,
            timeline: Option<TimelineName>,
        }

        let cases = [
            Case {
                name: "populated batch, known timeline",
                batch: populated_batch(),
                timeline: Some(TimelineName::from("frame")),
            },
            Case {
                name: "populated batch, unknown timeline",
                batch: populated_batch(),
                timeline: None,
            },
            Case {
                name: "empty batch, known timeline",
                batch: ChunkCandidatesDataframe::empty_record_batch(),
                timeline: Some(TimelineName::from("frame")),
            },
            Case {
                name: "empty batch, unknown timeline",
                batch: ChunkCandidatesDataframe::empty_record_batch(),
                timeline: None,
            },
        ];

        for case in cases {
            let original = case.batch.clone();
            let encoded = encode_legacy_candidates(case.batch, case.timeline.as_ref())
                .unwrap_or_else(|err| panic!("{}: {err}", case.name));

            // Column order: the ten base columns, in `QueryDatasetDataframe::min_schema` order,
            // plus the legacy start column when the timeline is known.
            let mut expected_names: Vec<String> = QueryDatasetDataframe::min_schema()
                .fields()
                .iter()
                .map(|field| field.name().clone())
                .collect();
            if let Some(timeline) = &case.timeline {
                expected_names.push(format!("{}:start", timeline.as_str()));
            }
            let actual_names: Vec<String> = encoded
                .schema()
                .fields()
                .iter()
                .map(|field| field.name().clone())
                .collect();
            assert_eq!(actual_names, expected_names, "{}: column order", case.name);

            // The base columns must be the very same arrays as the input: no row values rebuilt.
            for field in QueryDatasetDataframe::min_schema().fields() {
                let original_array = original
                    .column_by_name(field.name())
                    .expect("column present in the original batch");
                let encoded_array = encoded
                    .column_by_name(field.name())
                    .expect("column present in the encoded batch");
                assert!(
                    Arc::ptr_eq(original_array, encoded_array),
                    "{}: column `{}` must reuse the original array",
                    case.name,
                    field.name()
                );
            }

            match &case.timeline {
                Some(timeline) => {
                    let expected_field =
                        QueryDatasetResponse::field_timeline_start(timeline.as_str());
                    let actual_field = encoded
                        .schema_ref()
                        .field_with_name(expected_field.name())
                        .expect("start field present");
                    assert_eq!(
                        actual_field,
                        expected_field.as_ref(),
                        "{}: legacy field metadata",
                        case.name
                    );

                    let original_array = original
                        .column_by_name(ChunkCandidatesDataframe::COLUMN_CHUNK_TIMELINE_START_NAME)
                        .expect("chunk_timeline_start present in the original batch");
                    let encoded_array = encoded
                        .column_by_name(expected_field.name())
                        .expect("start column present in the encoded batch");
                    assert!(
                        Arc::ptr_eq(original_array, encoded_array),
                        "{}: start column must reuse the original array",
                        case.name
                    );

                    if original.num_rows() > 0 {
                        let start = encoded_array
                            .as_any()
                            .downcast_ref::<Int64Array>()
                            .expect("chunk_timeline_start is Int64Array");
                        assert_eq!(start.value(0), 5, "{}: start value preserved", case.name);
                        assert!(start.is_null(1), "{}: null start preserved", case.name);
                    }
                }
                None => {
                    assert!(
                        encoded
                            .column_by_name(
                                ChunkCandidatesDataframe::COLUMN_CHUNK_TIMELINE_START_NAME
                            )
                            .is_none(),
                        "{}: no start column when the timeline is unknown",
                        case.name
                    );
                }
            }

            if original.num_rows() > 0 {
                let byte_size = encoded
                    .column_by_name("chunk_byte_size_uncompressed")
                    .expect("column present")
                    .as_any()
                    .downcast_ref::<arrow::array::UInt64Array>()
                    .expect("chunk_byte_size_uncompressed is UInt64Array");
                assert_eq!(byte_size.value(0), 10, "{}: value preserved", case.name);
                assert!(byte_size.is_null(1), "{}: null preserved", case.name);
            }
        }
    }
}
