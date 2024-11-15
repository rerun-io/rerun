//! This crate contains generated types for the remote store gRPC service API.
//! Generation is done using the `re_protos_builder` crate.
//!
//! We want clear separation between 'internal' types and gRPC types and don't want
//! to use gRPC types in the rerun viewer codebase. That's why we implement all the
//! necessary conversion code (in the form of `From` and `TryFrom` traits) in this crate.
//!

/// Codec for serializing and deserializing query response (record batch) data
pub mod codec;

/// Generated types for the remote store gRPC service API v0.
pub mod v0 {
    // Ignoring all warnings for the auto-generated code.
    #[allow(clippy::doc_markdown)]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[allow(clippy::enum_variant_names)]
    #[allow(clippy::unwrap_used)]
    #[allow(clippy::wildcard_imports)]
    #[allow(clippy::manual_is_variant_and)]
    #[path = "../v0/rerun.remote_store.v0.rs"]
    mod _v0;

    pub use self::_v0::*;

    // ==== below are all necessary transforms from internal rerun types to protobuf types =====

    use std::{collections::BTreeSet, sync::Arc};

    #[derive(Debug, thiserror::Error)]
    pub enum TypeConversionError {
        #[error("missing required field: {0}")]
        MissingField(&'static str),
    }

    impl From<RecordingId> for re_log_types::StoreId {
        #[inline]
        fn from(value: RecordingId) -> Self {
            Self {
                kind: re_log_types::StoreKind::Recording,
                id: Arc::new(value.id),
            }
        }
    }

    impl From<re_log_types::StoreId> for RecordingId {
        #[inline]
        fn from(value: re_log_types::StoreId) -> Self {
            Self {
                id: value.id.to_string(),
            }
        }
    }

    impl From<re_log_types::ResolvedTimeRange> for TimeRange {
        fn from(time_range: re_log_types::ResolvedTimeRange) -> Self {
            Self {
                start: time_range.min().as_i64(),
                end: time_range.max().as_i64(),
            }
        }
    }

    impl TryFrom<Query> for re_dataframe::QueryExpression {
        type Error = TypeConversionError;

        fn try_from(value: Query) -> Result<Self, Self::Error> {
            let filtered_index = value
                .filtered_index
                .ok_or(TypeConversionError::MissingField("filtered_index"))?
                .try_into()?;

            let selection = value
                .column_selection
                .map(|cs| {
                    cs.columns
                        .into_iter()
                        .map(re_dataframe::ColumnSelector::try_from)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;

            let filtered_is_not_null = value
                .filtered_is_not_null
                .map(re_dataframe::ComponentColumnSelector::try_from)
                .transpose()?;

            Ok(Self {
                view_contents: value.view_contents.map(|vc| vc.into()),
                include_semantically_empty_columns: value.include_semantically_empty_columns,
                include_indicator_columns: value.include_indicator_columns,
                include_tombstone_columns: value.include_tombstone_columns,
                filtered_index: Some(filtered_index),
                filtered_index_range: value
                    .filtered_index_range
                    .map(|ir| ir.try_into())
                    .transpose()?,
                filtered_index_values: value
                    .filtered_index_values
                    .map(|iv| iv.time_points.into_iter().map(|v| v.into()).collect()),
                using_index_values: value
                    .using_index_values
                    .map(|uiv| uiv.time_points.into_iter().map(|v| v.into()).collect()),
                filtered_is_not_null,
                sparse_fill_strategy: re_dataframe::SparseFillStrategy::default(), // TODO(zehiko) implement support for sparse fill strategy
                selection,
            })
        }
    }

    impl From<ViewContents> for re_dataframe::ViewContentsSelector {
        fn from(value: ViewContents) -> Self {
            value
                .contents
                .into_iter()
                .map(|part| {
                    #[allow(clippy::unwrap_used)] // TODO(zehiko)
                    let entity_path = Into::<re_log_types::EntityPath>::into(part.path.unwrap());
                    let column_selector = part.components.map(|cs| {
                        cs.components
                            .into_iter()
                            .map(|c| re_dataframe::external::re_chunk::ComponentName::new(&c.name))
                            .collect::<BTreeSet<_>>()
                    });
                    (entity_path, column_selector)
                })
                .collect::<Self>()
        }
    }

    impl From<EntityPath> for re_log_types::EntityPath {
        fn from(value: EntityPath) -> Self {
            Self::from(value.path)
        }
    }

    impl TryFrom<IndexColumnSelector> for re_log_types::Timeline {
        type Error = TypeConversionError;

        fn try_from(value: IndexColumnSelector) -> Result<Self, Self::Error> {
            let timeline_name = value
                .timeline
                .ok_or(TypeConversionError::MissingField("timeline"))?
                .name;

            // TODO(cmc): QueryExpression::filtered_index gotta be a selector
            #[allow(clippy::match_same_arms)]
            let timeline = match timeline_name.as_str() {
                "log_time" => Self::new_temporal(timeline_name),
                "log_tick" => Self::new_sequence(timeline_name),
                "frame" => Self::new_sequence(timeline_name),
                "frame_nr" => Self::new_sequence(timeline_name),
                _ => Self::new_temporal(timeline_name),
            };

            Ok(timeline)
        }
    }

    impl TryFrom<IndexRange> for re_dataframe::IndexRange {
        type Error = TypeConversionError;

        fn try_from(value: IndexRange) -> Result<Self, Self::Error> {
            let time_range = value
                .time_range
                .ok_or(TypeConversionError::MissingField("time_range"))?;

            Ok(Self::new(time_range.start, time_range.end))
        }
    }

    impl From<TimeInt> for re_log_types::TimeInt {
        fn from(value: TimeInt) -> Self {
            Self::new_temporal(value.time)
        }
    }

    impl TryFrom<ComponentColumnSelector> for re_dataframe::ComponentColumnSelector {
        type Error = TypeConversionError;

        fn try_from(value: ComponentColumnSelector) -> Result<Self, Self::Error> {
            let entity_path = value
                .entity_path
                .ok_or(TypeConversionError::MissingField("entity_path"))?
                .into();

            let component_name = value
                .component
                .ok_or(TypeConversionError::MissingField("component"))?
                .name;

            Ok(Self {
                entity_path,
                component_name,
            })
        }
    }

    impl TryFrom<TimeColumnSelector> for re_dataframe::TimeColumnSelector {
        type Error = TypeConversionError;

        fn try_from(value: TimeColumnSelector) -> Result<Self, Self::Error> {
            let timeline = value
                .timeline
                .ok_or(TypeConversionError::MissingField("timeline"))?;

            Ok(Self {
                timeline: timeline.name.into(),
            })
        }
    }

    impl TryFrom<ColumnSelector> for re_dataframe::ColumnSelector {
        type Error = TypeConversionError;

        fn try_from(value: ColumnSelector) -> Result<Self, Self::Error> {
            match value
                .selector_type
                .ok_or(TypeConversionError::MissingField("selector_type"))?
            {
                column_selector::SelectorType::ComponentColumn(component_column_selector) => {
                    let selector: re_dataframe::ComponentColumnSelector =
                        component_column_selector.try_into()?;
                    Ok(selector.into())
                }
                column_selector::SelectorType::TimeColumn(time_column_selector) => {
                    let selector: re_dataframe::TimeColumnSelector =
                        time_column_selector.try_into()?;

                    Ok(selector.into())
                }
            }
        }
    }

    // ---- conversion from rerun's QueryExpression into protobuf Query ----

    impl From<re_dataframe::QueryExpression> for Query {
        fn from(value: re_dataframe::QueryExpression) -> Self {
            let view_contents = value
                .view_contents
                .map(|vc| {
                    vc.into_iter()
                        .map(|(path, components)| ViewContentsPart {
                            path: Some(path.into()),
                            components: components.map(|cs| ComponentsSet {
                                components: cs
                                    .into_iter()
                                    .map(|c| Component {
                                        name: c.to_string(),
                                    })
                                    .collect(),
                            }),
                        })
                        .collect::<Vec<_>>()
                })
                .map(|cs| ViewContents { contents: cs });

            Self {
                view_contents,
                include_semantically_empty_columns: value.include_semantically_empty_columns,
                include_indicator_columns: value.include_indicator_columns,
                include_tombstone_columns: value.include_tombstone_columns,
                filtered_index: value.filtered_index.map(|timeline| IndexColumnSelector {
                    timeline: Some(Timeline {
                        name: timeline.name().to_string(),
                    }),
                }),
                filtered_index_range: value.filtered_index_range.map(|ir| IndexRange {
                    time_range: Some(ir.into()),
                }),
                filtered_index_values: value.filtered_index_values.map(|iv| IndexValues {
                    time_points: iv
                        .into_iter()
                        // TODO(zehiko) is this desired behavior for TimeInt::STATIC?
                        .map(|v| TimeInt { time: v.as_i64() })
                        .collect(),
                }),
                using_index_values: value.using_index_values.map(|uiv| IndexValues {
                    time_points: uiv
                        .into_iter()
                        .map(|v| TimeInt { time: v.as_i64() })
                        .collect(),
                }),
                filtered_is_not_null: value.filtered_is_not_null.map(|cs| {
                    ComponentColumnSelector {
                        entity_path: Some(cs.entity_path.into()),
                        component: Some(Component {
                            name: cs.component_name,
                        }),
                    }
                }),
                column_selection: value.selection.map(|cs| ColumnSelection {
                    columns: cs.into_iter().map(|c| c.into()).collect(),
                }),
                sparse_fill_strategy: SparseFillStrategy::None.into(), // TODO(zehiko) implement
            }
        }
    }

    impl From<re_dataframe::EntityPath> for EntityPath {
        fn from(value: re_dataframe::EntityPath) -> Self {
            Self {
                path: value.to_string(),
            }
        }
    }

    impl From<re_dataframe::ColumnSelector> for ColumnSelector {
        fn from(value: re_dataframe::ColumnSelector) -> Self {
            match value {
                re_dataframe::ColumnSelector::Component(ccs) => Self {
                    selector_type: Some(column_selector::SelectorType::ComponentColumn(
                        ComponentColumnSelector {
                            entity_path: Some(ccs.entity_path.into()),
                            component: Some(Component {
                                name: ccs.component_name,
                            }),
                        },
                    )),
                },
                re_dataframe::ColumnSelector::Time(tcs) => Self {
                    selector_type: Some(column_selector::SelectorType::TimeColumn(
                        TimeColumnSelector {
                            timeline: Some(Timeline {
                                name: tcs.timeline.to_string(),
                            }),
                        },
                    )),
                },
            }
        }
    }

    // ------- Application level errors -------
    impl std::error::Error for RemoteStoreError {}

    impl std::fmt::Display for RemoteStoreError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!(
                "Remote store error. Request identifier: {}, error msg: {}, error code: {}",
                self.id, self.message, self.code
            ))
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::v0::{
        column_selector::SelectorType, ColumnSelection, ColumnSelector, Component,
        ComponentColumnSelector, ComponentsSet, EntityPath, IndexColumnSelector, IndexRange,
        IndexValues, Query, RecordingId, SparseFillStrategy, TimeInt, TimeRange, Timeline,
        ViewContents, ViewContentsPart,
    };

    #[test]
    pub fn test_query_conversion() {
        let grpc_query_before = Query {
            view_contents: Some(ViewContents {
                contents: vec![ViewContentsPart {
                    path: Some(EntityPath {
                        path: "/somepath".to_owned(),
                    }),
                    components: Some(ComponentsSet {
                        components: vec![Component {
                            name: "component".to_owned(),
                        }],
                    }),
                }],
            }),
            include_indicator_columns: false,
            include_semantically_empty_columns: true,
            include_tombstone_columns: true,
            filtered_index: Some(IndexColumnSelector {
                timeline: Some(Timeline {
                    name: "log_time".to_owned(),
                }),
            }),
            filtered_index_range: Some(IndexRange {
                time_range: Some(TimeRange { start: 0, end: 100 }),
            }),
            filtered_index_values: Some(IndexValues {
                time_points: vec![
                    TimeInt { time: 0 },
                    TimeInt { time: 1 },
                    TimeInt { time: 2 },
                ],
            }),
            using_index_values: Some(IndexValues {
                time_points: vec![
                    TimeInt { time: 3 },
                    TimeInt { time: 4 },
                    TimeInt { time: 5 },
                ],
            }),
            filtered_is_not_null: Some(ComponentColumnSelector {
                entity_path: Some(EntityPath {
                    path: "/somepath/c".to_owned(),
                }),
                component: Some(Component {
                    name: "component".to_owned(),
                }),
            }),
            column_selection: Some(ColumnSelection {
                columns: vec![ColumnSelector {
                    selector_type: Some(SelectorType::ComponentColumn(ComponentColumnSelector {
                        entity_path: Some(EntityPath {
                            path: "/somepath/c".to_owned(),
                        }),
                        component: Some(Component {
                            name: "component".to_owned(),
                        }),
                    })),
                }],
            }),
            sparse_fill_strategy: SparseFillStrategy::None.into(),
        };

        let query_expression_native: re_dataframe::QueryExpression =
            grpc_query_before.clone().try_into().unwrap();
        let grpc_query_after = query_expression_native.into();

        assert_eq!(grpc_query_before, grpc_query_after);
    }

    #[test]
    fn test_recording_id_conversion() {
        let recording_id = RecordingId {
            id: "recording_id".to_owned(),
        };

        let store_id: re_log_types::StoreId = recording_id.clone().into();
        let recording_id_after: RecordingId = store_id.into();

        assert_eq!(recording_id, recording_id_after);
    }
}
