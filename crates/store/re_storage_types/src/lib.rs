// Ignoring all warnings for generated code.
#![allow(clippy::doc_markdown)]
#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::manual_is_variant_and)]

pub mod v0 {

    // Some archetypes (e.g. `Clear`) are so fundamental and used everywhere that we want
    // them to be exposed by `re_types_core` directly; that way we don't force a dependency on the
    // `re_types` behemoth just so one can use one of these fundamental types.
    //
    // To do so, re-inject `re_types_core`'s archetypes into our own module.

    #[path = "../v0/rerun.storage.v0.rs"]
    mod _v0;

    pub use self::_v0::*;

    // ==== below are all necessary transforms from internal rerun types to protobuf types =====

    use std::collections::BTreeSet;

    impl From<re_log_types::ResolvedTimeRange> for TimeRange {
        fn from(rtr: re_log_types::ResolvedTimeRange) -> Self {
            Self {
                start: rtr.min().as_i64(),
                end: rtr.max().as_i64(),
            }
        }
    }

    impl From<Query> for re_dataframe2::external::re_chunk_store::QueryExpression2 {
        fn from(value: Query) -> Self {
            Self {
                view_contents: value.view_contents.map(|vc| vc.into()),
                // FIXME we need a consistent way on both sides to deal with the fact
                // prost generates Option<> for all nested fields, but some are required
                // See https://github.com/tokio-rs/prost/issues/223
                // We could do this at the client layer where we ensure required fields are set
                filtered_index: value.index.unwrap().into(),
                filtered_index_range: value.index_range.map(|ir| ir.into()),
                filtered_index_values: None,  // TODO implement
                sampled_index_values: None,   // TODO implement
                filtered_point_of_view: None, // TODO implement
                sparse_fill_strategy:
                    re_dataframe2::external::re_chunk_store::SparseFillStrategy::default(), // TODO implement,
                selection: value
                    .column_selection
                    .map(|cs| cs.columns.into_iter().map(|c| c.into()).collect()),
            }
        }
    }

    impl From<ViewContents> for re_dataframe2::external::re_chunk_store::ViewContentsSelector {
        fn from(value: ViewContents) -> Self {
            value
                .contents
                .into_iter()
                .map(|part| {
                    // FIXME option unwrap
                    let entity_path = Into::<re_log_types::EntityPath>::into(part.path.unwrap());
                    let column_selector = part.components.map(|cs| {
                        cs.values
                            .into_iter()
                            .map(|c| re_dataframe2::external::re_chunk::ComponentName::new(&c))
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

    impl From<IndexColumnSelector> for re_log_types::Timeline {
        fn from(value: IndexColumnSelector) -> Self {
            #![allow(clippy::match_same_arms)]
            let timeline = match value.name.as_str() {
                "log_time" => Self::new_temporal(value.name),
                "log_tick" => Self::new_sequence(value.name),
                "frame" => Self::new_sequence(value.name),
                "frame_nr" => Self::new_sequence(value.name),
                _ => Self::new_temporal(value.name),
            };

            timeline
        }
    }

    impl From<FilteredIndexRange> for re_dataframe2::external::re_chunk_store::IndexRange {
        fn from(value: FilteredIndexRange) -> Self {
            Self::new(
                // FIXME option unwrap
                value.time_range.unwrap().start,
                value.time_range.unwrap().end,
            )
        }
    }

    impl From<ColumnSelector> for re_dataframe2::external::re_chunk_store::ColumnSelector {
        fn from(value: ColumnSelector) -> Self {
            // FIXME option unwraps
            match value.selector_type.unwrap() {
                column_selector::SelectorType::ControlColumn(control_column_selector) => {
                    re_dataframe2::external::re_chunk_store::ControlColumnSelector {
                        component: re_dataframe2::external::re_chunk::ComponentName::new(
                            &control_column_selector.component,
                        ),
                    }
                    .into()
                }
                column_selector::SelectorType::ComponentColumn(component_column_selector) => {
                    re_dataframe2::external::re_chunk_store::ComponentColumnSelector {
                        entity_path: Into::<re_log_types::EntityPath>::into(
                            component_column_selector.entity_path.unwrap(),
                        ),
                        component: re_dataframe2::external::re_chunk::ComponentName::new(
                            &component_column_selector.component,
                        ),
                        join_encoding:
                            re_dataframe2::external::re_chunk_store::JoinEncoding::default(), // TODO implement
                    }
                    .into()
                }
                column_selector::SelectorType::TimeColumn(time_column_selector) => {
                    re_dataframe2::external::re_chunk_store::TimeColumnSelector {
                        timeline: time_column_selector.timeline_name.into(),
                    }
                    .into()
                }
            }
        }
    }
}
