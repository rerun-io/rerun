// Ignoring all warnings for the auto-generated code.
#![allow(clippy::doc_markdown)]
#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::manual_is_variant_and)]
pub mod v0 {
    #[path = "../v0/rerun.remote_store.v0.rs"]
    mod _v0;

    pub use self::_v0::*;

    // ==== below are all necessary transforms from internal rerun types to protobuf types =====

    use std::collections::BTreeSet;

    #[derive(Debug, thiserror::Error)]
    pub enum TypeConversionError {
        #[error("missing required field: {0}")]
        MissingField(&'static str),
    }

    impl From<re_log_types::ResolvedTimeRange> for TimeRange {
        fn from(time_range: re_log_types::ResolvedTimeRange) -> Self {
            Self {
                start: time_range.min().as_i64(),
                end: time_range.max().as_i64(),
            }
        }
    }

    impl TryFrom<Query> for re_dataframe::external::re_chunk_store::QueryExpression {
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
                        .map(|c| {
                            re_dataframe::external::re_chunk_store::ColumnSelector::try_from(c)
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;

            let filtered_point_of_view = value
                .filtered_pov
                .map(|fp| {
                    re_dataframe::external::re_chunk_store::ComponentColumnSelector::try_from(fp)
                })
                .transpose()?;

            Ok(Self {
                view_contents: value.view_contents.map(|vc| vc.into()),
                filtered_index,
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
                filtered_point_of_view,
                sparse_fill_strategy:
                    re_dataframe::external::re_chunk_store::SparseFillStrategy::default(), // TODO(zehiko) implement support for sparse fill strategy
                selection,
            })
        }
    }

    impl From<ViewContents> for re_dataframe::external::re_chunk_store::ViewContentsSelector {
        fn from(value: ViewContents) -> Self {
            value
                .contents
                .into_iter()
                .map(|part| {
                    // TODO(zehiko) option unwrap
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

    impl TryFrom<IndexRange> for re_dataframe::external::re_chunk_store::IndexRange {
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

    impl TryFrom<ComponentColumnSelector>
        for re_dataframe::external::re_chunk_store::ComponentColumnSelector
    {
        type Error = TypeConversionError;

        fn try_from(value: ComponentColumnSelector) -> Result<Self, Self::Error> {
            let entity_path = value
                .entity_path
                .ok_or(TypeConversionError::MissingField("entity_path"))?
                .into();

            let component = value
                .component
                .ok_or(TypeConversionError::MissingField("component"))?
                .name;

            Ok(Self {
                entity_path,
                component: re_dataframe::external::re_chunk::ComponentName::new(&component),
                join_encoding: re_dataframe::external::re_chunk_store::JoinEncoding::default(), // TODO(zehiko) implement
            })
        }
    }

    impl TryFrom<TimeColumnSelector> for re_dataframe::external::re_chunk_store::TimeColumnSelector {
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

    impl TryFrom<ColumnSelector> for re_dataframe::external::re_chunk_store::ColumnSelector {
        type Error = TypeConversionError;

        fn try_from(value: ColumnSelector) -> Result<Self, Self::Error> {
            match value
                .selector_type
                .ok_or(TypeConversionError::MissingField("selector_type"))?
            {
                column_selector::SelectorType::ComponentColumn(component_column_selector) => {
                    let selector: re_dataframe::external::re_chunk_store::ComponentColumnSelector =
                        component_column_selector.try_into()?;
                    Ok(selector.into())
                }
                column_selector::SelectorType::TimeColumn(time_column_selector) => {
                    let selector: re_dataframe::external::re_chunk_store::TimeColumnSelector =
                        time_column_selector.try_into()?;

                    Ok(selector.into())
                }
            }
        }
    }
}
