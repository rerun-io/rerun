use std::collections::{BTreeMap, BTreeSet};

use re_protos::{missing_field, TypeConversionError};
use re_sorbet::{ColumnSelector, ComponentColumnSelector};

impl TryFrom<re_protos::common::v1alpha1::ViewContents> for crate::ViewContentsSelector {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v1alpha1::ViewContents) -> Result<Self, Self::Error> {
        value
            .contents
            .into_iter()
            .map(|part| {
                let entity_path: re_log_types::EntityPath = part
                    .path
                    .ok_or(missing_field!(
                        re_protos::common::v1alpha1::ViewContentsPart,
                        "path",
                    ))?
                    .try_into()?;
                let column_selector = part.components.map(|cs| {
                    cs.components
                        .into_iter()
                        .map(|c| re_chunk::ComponentName::new(&c.name))
                        .collect::<BTreeSet<_>>()
                });
                Ok((entity_path, column_selector))
            })
            .collect::<Result<BTreeMap<_, _>, Self::Error>>()
            .map(crate::ViewContentsSelector)
    }
}

impl TryFrom<re_protos::common::v1alpha1::Query> for crate::QueryExpression {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v1alpha1::Query) -> Result<Self, Self::Error> {
        let filtered_index = value
            .filtered_index
            .ok_or(missing_field!(
                re_protos::common::v1alpha1::Query,
                "filtered_index",
            ))?
            .try_into()?;

        let selection = value
            .column_selection
            .map(|cs| {
                cs.columns
                    .into_iter()
                    .map(ColumnSelector::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        let filtered_is_not_null = value
            .filtered_is_not_null
            .map(ComponentColumnSelector::try_from)
            .transpose()?;

        Ok(Self {
            view_contents: value.view_contents.map(|vc| vc.try_into()).transpose()?,
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
            sparse_fill_strategy: crate::SparseFillStrategy::default(), // TODO(zehiko) implement support for sparse fill strategy
            selection,
        })
    }
}

impl From<crate::QueryExpression> for re_protos::common::v1alpha1::Query {
    fn from(value: crate::QueryExpression) -> Self {
        Self {
            view_contents: value
                .view_contents
                .map(|vc| {
                    vc.into_inner()
                        .into_iter()
                        .map(
                            |(path, components)| re_protos::common::v1alpha1::ViewContentsPart {
                                path: Some(path.into()),
                                components: components.map(|cs| {
                                    re_protos::common::v1alpha1::ComponentsSet {
                                        components: cs
                                            .into_iter()
                                            .map(|c| re_protos::common::v1alpha1::Component {
                                                name: c.to_string(),
                                            })
                                            .collect(),
                                    }
                                }),
                            },
                        )
                        .collect::<Vec<_>>()
                })
                .map(|cs| re_protos::common::v1alpha1::ViewContents { contents: cs }),
            include_semantically_empty_columns: value.include_semantically_empty_columns,
            include_indicator_columns: value.include_indicator_columns,
            include_tombstone_columns: value.include_tombstone_columns,
            filtered_index: value.filtered_index.map(|index_name| {
                re_protos::common::v1alpha1::IndexColumnSelector {
                    timeline: Some(re_protos::common::v1alpha1::Timeline {
                        name: index_name.to_string(),
                    }),
                }
            }),
            filtered_index_range: value.filtered_index_range.map(|ir| {
                re_protos::common::v1alpha1::IndexRange {
                    time_range: Some(ir.into()),
                }
            }),
            filtered_index_values: value.filtered_index_values.map(|iv| {
                re_protos::common::v1alpha1::IndexValues {
                    time_points: iv
                        .into_iter()
                        // TODO(zehiko) is this desired behavior for TimeInt::STATIC?
                        .map(|v| re_protos::common::v1alpha1::TimeInt { time: v.as_i64() })
                        .collect(),
                }
            }),
            using_index_values: value.using_index_values.map(|uiv| {
                re_protos::common::v1alpha1::IndexValues {
                    time_points: uiv
                        .into_iter()
                        .map(|v| re_protos::common::v1alpha1::TimeInt { time: v.as_i64() })
                        .collect(),
                }
            }),
            filtered_is_not_null: value.filtered_is_not_null.map(|cs| {
                re_protos::common::v1alpha1::ComponentColumnSelector {
                    entity_path: Some(cs.entity_path.into()),
                    component: Some(re_protos::common::v1alpha1::Component {
                        name: cs.component_name,
                    }),
                }
            }),
            column_selection: value.selection.map(|cs| {
                re_protos::common::v1alpha1::ColumnSelection {
                    columns: cs.into_iter().map(|c| c.into()).collect(),
                }
            }),
            sparse_fill_strategy: re_protos::common::v1alpha1::SparseFillStrategy::None.into(), // TODO(zehiko) implement
        }
    }
}

#[cfg(test)]
mod tests {
    use re_protos::common::v1alpha1::{
        column_selector::SelectorType, ColumnSelection, ColumnSelector, Component,
        ComponentColumnSelector, ComponentsSet, EntityPath, IndexColumnSelector, IndexRange,
        IndexValues, Query, SparseFillStrategy, TimeInt, TimeRange, Timeline, ViewContents,
        ViewContentsPart,
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

        let query_expression_native: crate::QueryExpression =
            grpc_query_before.clone().try_into().unwrap();
        let grpc_query_after = query_expression_native.into();

        assert_eq!(grpc_query_before, grpc_query_after);
    }
}
