//! This crate contains generated types for the remote store gRPC service API.
//! Generation is done using the `re_protos_builder` crate.
//!
//! We want clear separation between 'internal' types and gRPC types and don't want
//! to use gRPC types in the rerun viewer codebase. That's why we implement all the
//! necessary conversion code (in the form of `From` and `TryFrom` traits) in this crate.
//!

// This extra module is needed, because of how imports from different packages are resolved.
// For example, `rerun.common.v0.EncoderVersion` is resolved to `super::super::common::v0::EncoderVersion`.
// We need an extra module in the path to `common` to make that work.
// Additionally, the `common` module itself has to exist with a `v0` module inside of it,
// which is the reason for the `common`, `log_msg`, `remote_store`, etc. modules below.

pub mod external {
    pub use prost;
}

// Note: Be careful with `#[path]` attributes: https://github.com/rust-lang/rust/issues/35016
mod v0 {
    // Note: `allow(clippy::all)` does NOT allow all lints
    #![allow(clippy::all, clippy::pedantic, clippy::nursery)]

    #[path = "./rerun.common.v0.rs"]
    pub mod rerun_common_v0;

    #[path = "./rerun.log_msg.v0.rs"]
    pub mod rerun_log_msg_v0;

    #[path = "./rerun.remote_store.v0.rs"]
    pub mod rerun_remote_store_v0;
}

pub mod common {
    pub mod v0 {
        pub use crate::v0::rerun_common_v0::*;
    }
}

pub mod log_msg {
    pub mod v0 {
        pub use crate::v0::rerun_log_msg_v0::*;
    }
}

/// Generated types for the remote store gRPC service API v0.
pub mod remote_store {
    pub mod v0 {
        pub use crate::v0::rerun_remote_store_v0::*;
    }
}

// // ==== below are all necessary transforms from internal rerun types to protobuf types =====

// use std::{collections::BTreeSet, sync::Arc};

#[derive(Debug, thiserror::Error)]
pub enum TypeConversionError {
    #[error("missing required field: {type_name}.{field_name}")]
    MissingField {
        type_name: &'static str,
        field_name: &'static str,
    },

    #[error("invalid value for field {type_name}.{field_name}: {reason}")]
    InvalidField {
        type_name: &'static str,
        field_name: &'static str,
        reason: String,
    },

    #[error("failed to decode: {0}")]
    DecodeError(#[from] prost::DecodeError),

    #[error("failed to encode: {0}")]
    EncodeError(#[from] prost::EncodeError),

    #[error("{0}")]
    UnknownEnumValue(#[from] prost::UnknownEnumValue),
}

impl TypeConversionError {
    pub fn missing_field(type_name: &'static str, field_name: &'static str) -> Self {
        Self::MissingField {
            type_name,
            field_name,
        }
    }
}

// // ---- conversion from rerun's QueryExpression into protobuf Query ----

// impl From<re_dataframe::QueryExpression> for Query {
//     fn from(value: re_dataframe::QueryExpression) -> Self {
//         let view_contents = value
//             .view_contents
//             .map(|vc| {
//                 vc.into_iter()
//                     .map(|(path, components)| ViewContentsPart {
//                         path: Some(path.into()),
//                         components: components.map(|cs| ComponentsSet {
//                             components: cs
//                                 .into_iter()
//                                 .map(|c| Component {
//                                     name: c.to_string(),
//                                 })
//                                 .collect(),
//                         }),
//                     })
//                     .collect::<Vec<_>>()
//             })
//             .map(|cs| ViewContents { contents: cs });

//         Self {
//             view_contents,
//             include_semantically_empty_columns: value.include_semantically_empty_columns,
//             include_indicator_columns: value.include_indicator_columns,
//             include_tombstone_columns: value.include_tombstone_columns,
//             filtered_index: value.filtered_index.map(|timeline| IndexColumnSelector {
//                 timeline: Some(Timeline {
//                     name: timeline.name().to_string(),
//                 }),
//             }),
//             filtered_index_range: value.filtered_index_range.map(|ir| IndexRange {
//                 time_range: Some(ir.into()),
//             }),
//             filtered_index_values: value.filtered_index_values.map(|iv| IndexValues {
//                 time_points: iv
//                     .into_iter()
//                     // TODO(zehiko) is this desired behavior for TimeInt::STATIC?
//                     .map(|v| TimeInt { time: v.as_i64() })
//                     .collect(),
//             }),
//             using_index_values: value.using_index_values.map(|uiv| IndexValues {
//                 time_points: uiv
//                     .into_iter()
//                     .map(|v| TimeInt { time: v.as_i64() })
//                     .collect(),
//             }),
//             filtered_is_not_null: value
//                 .filtered_is_not_null
//                 .map(|cs| ComponentColumnSelector {
//                     entity_path: Some(cs.entity_path.into()),
//                     component: Some(Component {
//                         name: cs.component_name,
//                     }),
//                 }),
//             column_selection: value.selection.map(|cs| ColumnSelection {
//                 columns: cs.into_iter().map(|c| c.into()).collect(),
//             }),
//             sparse_fill_strategy: SparseFillStrategy::None.into(), // TODO(zehiko) implement
//         }
//     }
// }

// // ------- Application level errors -------
// impl std::error::Error for RegistrationError {}

// impl std::fmt::Display for RegistrationError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.write_fmt(format_args!(
//             "Failed to register recording: {}, error code: {}, error message: {}",
//             self.storage_url, self.code, self.message
//         ))
//     }
// }

// #[cfg(test)]
// mod tests {

//     use crate::v0::{
//         column_selector::SelectorType, ColumnSelection, ColumnSelector, Component,
//         ComponentColumnSelector, ComponentsSet, EntityPath, IndexColumnSelector, IndexRange,
//         IndexValues, Query, RecordingId, SparseFillStrategy, TimeInt, TimeRange, Timeline,
//         ViewContents, ViewContentsPart,
//     };

//     #[test]
//     pub fn test_query_conversion() {
//         let grpc_query_before = Query {
//             view_contents: Some(ViewContents {
//                 contents: vec![ViewContentsPart {
//                     path: Some(EntityPath {
//                         path: "/somepath".to_owned(),
//                     }),
//                     components: Some(ComponentsSet {
//                         components: vec![Component {
//                             name: "component".to_owned(),
//                         }],
//                     }),
//                 }],
//             }),
//             include_indicator_columns: false,
//             include_semantically_empty_columns: true,
//             include_tombstone_columns: true,
//             filtered_index: Some(IndexColumnSelector {
//                 timeline: Some(Timeline {
//                     name: "log_time".to_owned(),
//                 }),
//             }),
//             filtered_index_range: Some(IndexRange {
//                 time_range: Some(TimeRange { start: 0, end: 100 }),
//             }),
//             filtered_index_values: Some(IndexValues {
//                 time_points: vec![
//                     TimeInt { time: 0 },
//                     TimeInt { time: 1 },
//                     TimeInt { time: 2 },
//                 ],
//             }),
//             using_index_values: Some(IndexValues {
//                 time_points: vec![
//                     TimeInt { time: 3 },
//                     TimeInt { time: 4 },
//                     TimeInt { time: 5 },
//                 ],
//             }),
//             filtered_is_not_null: Some(ComponentColumnSelector {
//                 entity_path: Some(EntityPath {
//                     path: "/somepath/c".to_owned(),
//                 }),
//                 component: Some(Component {
//                     name: "component".to_owned(),
//                 }),
//             }),
//             column_selection: Some(ColumnSelection {
//                 columns: vec![ColumnSelector {
//                     selector_type: Some(SelectorType::ComponentColumn(ComponentColumnSelector {
//                         entity_path: Some(EntityPath {
//                             path: "/somepath/c".to_owned(),
//                         }),
//                         component: Some(Component {
//                             name: "component".to_owned(),
//                         }),
//                     })),
//                 }],
//             }),
//             sparse_fill_strategy: SparseFillStrategy::None.into(),
//         };

//         let query_expression_native: re_dataframe::QueryExpression =
//             grpc_query_before.clone().try_into().unwrap();
//         let grpc_query_after = query_expression_native.into();

//         assert_eq!(grpc_query_before, grpc_query_after);
//     }
// }
