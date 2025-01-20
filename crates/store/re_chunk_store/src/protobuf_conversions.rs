use re_protos::invalid_field;
use re_protos::missing_field;
use re_protos::TypeConversionError;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

impl TryFrom<re_protos::common::v0::ComponentColumnSelector> for crate::ComponentColumnSelector {
    type Error = TypeConversionError;

    fn try_from(
        value: re_protos::common::v0::ComponentColumnSelector,
    ) -> Result<Self, Self::Error> {
        let entity_path = value
            .entity_path
            .ok_or(missing_field!(
                re_protos::common::v0::ComponentColumnSelector,
                "entity_path",
            ))?
            .try_into()?;

        let component_name = value
            .component
            .ok_or(missing_field!(
                re_protos::common::v0::ComponentColumnSelector,
                "component",
            ))?
            .name;

        Ok(Self {
            entity_path,
            component_name,
        })
    }
}

impl TryFrom<re_protos::common::v0::TimeColumnSelector> for crate::TimeColumnSelector {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::TimeColumnSelector) -> Result<Self, Self::Error> {
        let timeline = value.timeline.ok_or(missing_field!(
            re_protos::common::v0::TimeColumnSelector,
            "timeline",
        ))?;

        Ok(Self {
            timeline: timeline.name.into(),
        })
    }
}

impl TryFrom<re_protos::common::v0::ColumnSelector> for crate::ColumnSelector {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::ColumnSelector) -> Result<Self, Self::Error> {
        match value.selector_type.ok_or(missing_field!(
            re_protos::common::v0::ColumnSelector,
            "selector_type",
        ))? {
            re_protos::common::v0::column_selector::SelectorType::ComponentColumn(
                component_column_selector,
            ) => {
                let selector: crate::ComponentColumnSelector =
                    component_column_selector.try_into()?;
                Ok(selector.into())
            }
            re_protos::common::v0::column_selector::SelectorType::TimeColumn(
                time_column_selector,
            ) => {
                let selector: crate::TimeColumnSelector = time_column_selector.try_into()?;

                Ok(selector.into())
            }
        }
    }
}

impl From<crate::ColumnSelector> for re_protos::common::v0::ColumnSelector {
    fn from(value: crate::ColumnSelector) -> Self {
        match value {
            crate::ColumnSelector::Component(ccs) => Self {
                selector_type: Some(
                    re_protos::common::v0::column_selector::SelectorType::ComponentColumn(
                        re_protos::common::v0::ComponentColumnSelector {
                            entity_path: Some(ccs.entity_path.into()),
                            component: Some(re_protos::common::v0::Component {
                                name: ccs.component_name,
                            }),
                        },
                    ),
                ),
            },
            crate::ColumnSelector::Time(tcs) => Self {
                selector_type: Some(
                    re_protos::common::v0::column_selector::SelectorType::TimeColumn(
                        re_protos::common::v0::TimeColumnSelector {
                            timeline: Some(re_protos::common::v0::Timeline {
                                name: tcs.timeline.to_string(),
                            }),
                        },
                    ),
                ),
            },
        }
    }
}

impl TryFrom<re_protos::common::v0::ViewContents> for crate::ViewContentsSelector {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::ViewContents) -> Result<Self, Self::Error> {
        value
            .contents
            .into_iter()
            .map(|part| {
                let entity_path: re_log_types::EntityPath = part
                    .path
                    .ok_or(missing_field!(
                        re_protos::common::v0::ViewContentsPart,
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

impl TryFrom<re_protos::common::v0::Query> for crate::QueryExpression {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::Query) -> Result<Self, Self::Error> {
        let filtered_index = value
            .filtered_index
            .ok_or(missing_field!(
                re_protos::common::v0::Query,
                "filtered_index",
            ))?
            .try_into()?;

        let selection = value
            .column_selection
            .map(|cs| {
                cs.columns
                    .into_iter()
                    .map(crate::ColumnSelector::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        let filtered_is_not_null = value
            .filtered_is_not_null
            .map(crate::ComponentColumnSelector::try_from)
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

impl From<crate::QueryExpression> for re_protos::common::v0::Query {
    fn from(value: crate::QueryExpression) -> Self {
        Self {
            view_contents: value
                .view_contents
                .map(|vc| {
                    vc.into_inner()
                        .into_iter()
                        .map(
                            |(path, components)| re_protos::common::v0::ViewContentsPart {
                                path: Some(path.into()),
                                components: components.map(|cs| {
                                    re_protos::common::v0::ComponentsSet {
                                        components: cs
                                            .into_iter()
                                            .map(|c| re_protos::common::v0::Component {
                                                name: c.to_string(),
                                            })
                                            .collect(),
                                    }
                                }),
                            },
                        )
                        .collect::<Vec<_>>()
                })
                .map(|cs| re_protos::common::v0::ViewContents { contents: cs }),
            include_semantically_empty_columns: value.include_semantically_empty_columns,
            include_indicator_columns: value.include_indicator_columns,
            include_tombstone_columns: value.include_tombstone_columns,
            filtered_index: value.filtered_index.map(|timeline| {
                re_protos::common::v0::IndexColumnSelector {
                    timeline: Some(re_protos::common::v0::Timeline {
                        name: timeline.name().to_string(),
                    }),
                }
            }),
            filtered_index_range: value.filtered_index_range.map(|ir| {
                re_protos::common::v0::IndexRange {
                    time_range: Some(ir.into()),
                }
            }),
            filtered_index_values: value.filtered_index_values.map(|iv| {
                re_protos::common::v0::IndexValues {
                    time_points: iv
                        .into_iter()
                        // TODO(zehiko) is this desired behavior for TimeInt::STATIC?
                        .map(|v| re_protos::common::v0::TimeInt { time: v.as_i64() })
                        .collect(),
                }
            }),
            using_index_values: value.using_index_values.map(|uiv| {
                re_protos::common::v0::IndexValues {
                    time_points: uiv
                        .into_iter()
                        .map(|v| re_protos::common::v0::TimeInt { time: v.as_i64() })
                        .collect(),
                }
            }),
            filtered_is_not_null: value.filtered_is_not_null.map(|cs| {
                re_protos::common::v0::ComponentColumnSelector {
                    entity_path: Some(cs.entity_path.into()),
                    component: Some(re_protos::common::v0::Component {
                        name: cs.component_name,
                    }),
                }
            }),
            column_selection: value
                .selection
                .map(|cs| re_protos::common::v0::ColumnSelection {
                    columns: cs.into_iter().map(|c| c.into()).collect(),
                }),
            sparse_fill_strategy: re_protos::common::v0::SparseFillStrategy::None.into(), // TODO(zehiko) implement
        }
    }
}

impl TryFrom<crate::ColumnDescriptor> for re_protos::common::v0::ColumnDescriptor {
    type Error = TypeConversionError;

    fn try_from(value: crate::ColumnDescriptor) -> Result<Self, Self::Error> {
        match value {
            crate::ColumnDescriptor::Time(time_descriptor) => Ok(Self {
                descriptor_type: Some(
                    re_protos::common::v0::column_descriptor::DescriptorType::TimeColumn(
                        re_protos::common::v0::TimeColumnDescriptor {
                            timeline: Some(re_protos::common::v0::Timeline {
                                name: time_descriptor.timeline.name().to_string(),
                            }),
                            datatype: time_descriptor.datatype.to_string(), // TODO(emilk): use arrow IPC instead
                        },
                    ),
                ),
            }),
            crate::ColumnDescriptor::Component(component_descriptor) => Ok(Self {
                descriptor_type: Some(
                    re_protos::common::v0::column_descriptor::DescriptorType::ComponentColumn(
                        re_protos::common::v0::ComponentColumnDescriptor {
                            entity_path: Some(component_descriptor.entity_path.into()),
                            archetype_name: component_descriptor
                                .archetype_name
                                .map(|an| an.to_string()),
                            archetype_field_name: component_descriptor
                                .archetype_field_name
                                .map(|afn| afn.to_string()),
                            component_name: component_descriptor.component_name.to_string(),
                            datatype: component_descriptor.store_datatype.to_string(), // TODO(emilk): use arrow IPC instead
                            is_static: component_descriptor.is_static,
                            is_tombstone: component_descriptor.is_tombstone,
                            is_semantically_empty: component_descriptor.is_semantically_empty,
                            is_indicator: component_descriptor.is_indicator,
                        },
                    ),
                ),
            }),
        }
    }
}

impl TryFrom<re_protos::common::v0::ColumnDescriptor> for crate::ColumnDescriptor {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::ColumnDescriptor) -> Result<Self, Self::Error> {
        let descriptor = value.descriptor_type.ok_or(missing_field!(
            re_protos::common::v0::ColumnDescriptor,
            "descriptor_type",
        ))?;

        match descriptor {
            re_protos::common::v0::column_descriptor::DescriptorType::TimeColumn(
                time_descriptor,
            ) => Ok(Self::Time(crate::TimeColumnDescriptor {
                timeline: time_descriptor
                    .timeline
                    .ok_or(missing_field!(
                        re_protos::common::v0::TimeColumnDescriptor,
                        "timeline",
                    ))?
                    .into(),
                datatype: time_descriptor.datatype.parse().map_err(|err| {
                    invalid_field!(
                        re_protos::common::v0::ColumnDescriptor,
                        "time column descriptor",
                        err
                    )
                })?,
            })),
            re_protos::common::v0::column_descriptor::DescriptorType::ComponentColumn(
                component_descriptor,
            ) => Ok(Self::Component(crate::ComponentColumnDescriptor {
                entity_path: component_descriptor
                    .entity_path
                    .ok_or(missing_field!(
                        re_protos::common::v0::ComponentColumnDescriptor,
                        "entity_path",
                    ))?
                    .try_into()?,
                archetype_name: component_descriptor.archetype_name.map(Into::into),
                archetype_field_name: component_descriptor.archetype_field_name.map(Into::into),
                component_name: component_descriptor.component_name.into(),
                store_datatype: component_descriptor.datatype.parse().map_err(|err| {
                    invalid_field!(
                        re_protos::common::v0::ColumnDescriptor,
                        "component column descriptor",
                        err
                    )
                })?,
                is_static: component_descriptor.is_static,
                is_tombstone: component_descriptor.is_tombstone,
                is_semantically_empty: component_descriptor.is_semantically_empty,
                is_indicator: component_descriptor.is_indicator,
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use re_protos::common::v0::{
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

    #[test]
    fn test_time_column_descriptor_conversion() {
        let time_descriptor = crate::TimeColumnDescriptor {
            timeline: crate::Timeline::log_time(),
            datatype: arrow::datatypes::DataType::Timestamp(
                arrow::datatypes::TimeUnit::Nanosecond,
                None,
            ),
        };

        let descriptor = crate::ColumnDescriptor::Time(time_descriptor.clone());

        let proto_descriptor: re_protos::common::v0::ColumnDescriptor =
            descriptor.try_into().unwrap();
        let descriptor_after = proto_descriptor.try_into().unwrap();
        let crate::ColumnDescriptor::Time(time_descriptor_after) = descriptor_after else {
            panic!("Expected TimeColumnDescriptor")
        };

        assert_eq!(time_descriptor, time_descriptor_after);
    }

    #[test]
    fn test_component_column_descriptor_conversion() {
        let component_descriptor = crate::ComponentColumnDescriptor {
            entity_path: re_log_types::EntityPath::from("/some/path"),
            archetype_name: Some("archetype".to_owned().into()),
            archetype_field_name: Some("field".to_owned().into()),
            component_name: re_chunk::ComponentName::new("component"),
            store_datatype: arrow::datatypes::DataType::Int64,
            is_static: true,
            is_tombstone: false,
            is_semantically_empty: false,
            is_indicator: true,
        };

        let descriptor = crate::ColumnDescriptor::Component(component_descriptor.clone());

        let proto_descriptor: re_protos::common::v0::ColumnDescriptor =
            descriptor.try_into().unwrap();
        let descriptor_after = proto_descriptor.try_into().unwrap();
        let crate::ColumnDescriptor::Component(component_descriptor_after) = descriptor_after
        else {
            panic!("Expected ComponentColumnDescriptor")
        };

        assert_eq!(component_descriptor, component_descriptor_after);
    }
}
