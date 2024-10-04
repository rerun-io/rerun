#![allow(clippy::fallible_impl_from)] // see TODO below, we need to fix all Option unwraps
use std::collections::BTreeSet;

use re_dataframe2::external::re_chunk::ComponentName;

tonic::include_proto!("rerun.storage.v0");

// ==== below are all necessary conversions from internal rerun types to protobuf types =====

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
            // prost generates Option<> for all nested fields
            // See https://github.com/tokio-rs/prost/issues/223
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
                let entity_path = Into::<re_log_types::EntityPath>::into(part.path.unwrap());
                let column_selector = part
                    .components
                    .into_iter()
                    .map(|c| ComponentName::new(&c))
                    .collect::<BTreeSet<_>>();
                (entity_path, None)
            })
            .collect::<Self>()
    }
}

impl From<EntityPath> for re_log_types::EntityPath {
    fn from(value: EntityPath) -> Self {
        Self::from_single_string(value.path)
    }
}

impl From<IndexColumnSelector> for re_log_types::Timeline {
    fn from(value: IndexColumnSelector) -> Self {
        let timeline = match value.name.as_str() {
            "log_time" => re_log_types::Timeline::new_temporal(value.name),
            "log_tick" => re_log_types::Timeline::new_sequence(value.name),
            "frame" => re_log_types::Timeline::new_sequence(value.name),
            "frame_nr" => re_log_types::Timeline::new_sequence(value.name),
            _ => re_log_types::Timeline::new_temporal(value.name),
        };

        timeline
    }
}

impl From<FilteredIndexRange> for re_dataframe2::external::re_chunk_store::IndexRange {
    fn from(value: FilteredIndexRange) -> Self {
        Self::new(
            value.time_range.unwrap().start,
            value.time_range.unwrap().end,
        )
    }
}

impl From<ColumnSelector> for re_dataframe2::external::re_chunk_store::ColumnSelector {
    fn from(value: ColumnSelector) -> Self {
        match value.selector_type.unwrap() {
            column_selector::SelectorType::ControlColumn(control_column_selector) => {
                re_dataframe2::external::re_chunk_store::ControlColumnSelector {
                    component: ComponentName::new(&control_column_selector.component_name),
                }
                .into()
            }
            column_selector::SelectorType::ComponentColumn(component_column_selector) => {
                re_dataframe2::external::re_chunk_store::ComponentColumnSelector {
                    entity_path: Into::<re_log_types::EntityPath>::into(
                        component_column_selector.entity_path.unwrap(),
                    ),
                    component: ComponentName::new(&component_column_selector.component),
                    join_encoding: re_dataframe2::external::re_chunk_store::JoinEncoding::default(), // TODO implement
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
