use std::sync::Arc;

use re_chunk_store::RangeQuery;
use re_log_types::external::arrow::array::{ArrayRef, Float64Array, StringArray, StructArray};
use re_log_types::external::arrow::datatypes::Field;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, Timeline, TimelineName};
use re_sdk_types::archetypes::Measurements;
use re_sdk_types::{ComponentDescriptor, RowId};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view::BlueprintResolvedResultsExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{
    DragAndDropFeedback, DragAndDropPayload, ViewClass as _, ViewContext, VisualizerComponentSource,
};
use re_viewport_blueprint::ViewBlueprint;

#[test]
fn dropping_temperature_preserves_heuristic_mappings() {
    let source = "sensor_msgs.msg.Temperature:message";
    let entity = EntityPath::from("raw");
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    test_context.log_entity(entity.clone(), |builder| {
        builder.with_row(
            RowId::new(),
            [(Timeline::log_tick(), 1)],
            [(
                ComponentDescriptor::partial(source),
                structure([("variance", number(0.25)), ("temperature", number(41.0))]),
            )],
        )
    });
    let automatic_view = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TimeSeriesView::identifier(),
        ))
    });
    let expected = test_context.query_results[&automatic_view]
        .tree
        .lookup_result_by_path(entity.hash())
        .unwrap()
        .visualizer_instructions
        .clone();
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].component_mappings.len(), 3);

    let drop_view = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new(
            TimeSeriesView::identifier(),
            re_viewer_context::RecommendedView::new_single_entity(EntityPath::from("other")),
        ))
    });
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        assert_eq!(
            TimeSeriesView.handle_drop(
                &ViewBlueprint::try_from_db(drop_view, ctx.blueprint_db(), ctx.blueprint_query)
                    .expect("view should exist")
                    .bundle_context_with_state(ctx, TimeSeriesView.new_state().as_ref()),
                &DragAndDropPayload::Components {
                    component_paths: vec1::vec1![re_log_types::ComponentPath::new(
                        entity.clone(),
                        source.into()
                    )]
                },
                true,
            ),
            DragAndDropFeedback::Accept
        );
    });
    test_context.handle_system_commands(&egui::Context::default());
    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});
    let actual = &test_context.query_results[&drop_view]
        .tree
        .lookup_result_by_path(entity.hash())
        .unwrap()
        .visualizer_instructions;
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].visualizer_type, expected[0].visualizer_type);
    assert_eq!(actual[0].component_mappings, expected[0].component_mappings);

    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        for view in [automatic_view, drop_view] {
            assert_eq!(
                TimeSeriesView.handle_drop(
                    &ViewBlueprint::try_from_db(view, ctx.blueprint_db(), ctx.blueprint_query)
                        .expect("view should exist")
                        .bundle_context_with_state(ctx, TimeSeriesView.new_state().as_ref()),
                    &DragAndDropPayload::Components {
                        component_paths: vec1::vec1![re_log_types::ComponentPath::new(
                            entity.clone(),
                            source.into()
                        )]
                    },
                    false,
                ),
                DragAndDropFeedback::Reject(Some("Already visualized"))
            );
        }
    });

    // A different component on the same entity must not inherit the message's heuristic.
    test_context.log_entity(entity.clone(), |builder| {
        builder.with_archetype_auto_row(
            [(Timeline::log_tick(), 1)],
            &re_sdk_types::archetypes::Scalars::single(7.0),
        )
    });
    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});
    test_context.run_once_in_egui_central_panel(|ctx, _ui| {
        assert_eq!(
            TimeSeriesView.handle_drop(
                &ViewBlueprint::try_from_db(drop_view, ctx.blueprint_db(), ctx.blueprint_query)
                    .expect("view should exist")
                    .bundle_context_with_state(ctx, TimeSeriesView.new_state().as_ref()),
                &DragAndDropPayload::Components {
                    component_paths: vec1::vec1![re_log_types::ComponentPath::new(
                        entity.clone(),
                        re_sdk_types::archetypes::Scalars::descriptor_scalars().component,
                    )]
                },
                true,
            ),
            DragAndDropFeedback::Accept
        );
    });
    test_context.handle_system_commands(&egui::Context::default());
    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});
    let actual = &test_context.query_results[&drop_view]
        .tree
        .lookup_result_by_path(entity.hash())
        .unwrap()
        .visualizer_instructions;
    assert_eq!(actual.len(), 2);
    assert!(actual.iter().any(|instruction| instruction.visualizer_type
        == expected[0].visualizer_type
        && instruction.component_mappings == expected[0].component_mappings));
    assert!(actual.iter().any(|instruction| instruction.visualizer_type
        == re_viewer_context::ViewSystemIdentifier::from("SeriesLines")));
}

fn structure(fields: impl IntoIterator<Item = (&'static str, ArrayRef)>) -> ArrayRef {
    Arc::new(StructArray::from(
        fields
            .into_iter()
            .map(|(name, array)| {
                (
                    Arc::new(Field::new(name, array.data_type().clone(), false)),
                    array,
                )
            })
            .collect::<Vec<_>>(),
    ))
}

fn text(value: &str) -> ArrayRef {
    Arc::new(StringArray::from(vec![value]))
}

fn number(value: f64) -> ArrayRef {
    Arc::new(Float64Array::from(vec![value]))
}

// Integration test from ros-message all the way to blueprint resolved query resolution yielding the expected data.
#[test]
fn temperature_mappings_resolve_range_values_and_timestamps() {
    let source = "sensor_msgs.msg.Temperature:message";
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let entity_path = EntityPath::from("raw");
    let rows = [(1, RowId::new()), (2, RowId::new())];
    for (tick, row_id) in rows {
        let input = structure([
            ("temperature", number(40.0 + tick as f64)),
            ("variance", number(tick as f64 / 4.0)),
        ]);
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_row(
                row_id,
                [(Timeline::log_tick(), tick)],
                [(ComponentDescriptor::partial(source), input)],
            )
        });
    }
    let class = TimeSeriesView::identifier();
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(class))
    });
    let query_result = &test_context.query_results[&view_id];
    let data_result = query_result
        .tree
        .lookup_result_by_path(entity_path.hash())
        .unwrap();
    assert_eq!(data_result.visualizer_instructions.len(), 1);
    let instruction = &data_result.visualizer_instructions[0];
    let targets = [
        Measurements::descriptor_values().component,
        Measurements::descriptor_variances().component,
        Measurements::descriptor_units().component,
    ];
    assert_eq!(
        instruction.component_mappings,
        std::iter::zip(targets, [".temperature", ".variance", r#""°C""#])
            .map(|(target, selector)| (
                target,
                VisualizerComponentSource::SourceComponent {
                    source_component: source.into(),
                    selector: selector.into(),
                }
            ))
            .collect()
    );

    test_context.run(&egui::Context::default(), |viewer_ctx| {
        let ctx = ViewContext {
            viewer_ctx,
            view_id,
            view_class_identifier: class,
            space_origin: &EntityPath::root(),
            view_state: &(),
            query_result,
        };
        let range = re_view::range_with_blueprint_resolved_data(
            &ctx,
            None,
            &RangeQuery::new(TimelineName::log_tick(), AbsoluteTimeRange::new(1, 2)),
            data_result,
            targets,
            instruction,
        );
        let expected = [
            [number(41.0), number(42.0)],
            [number(0.25), number(0.5)],
            [text("°C"), text("°C")],
        ];
        for (target, expected) in std::iter::zip(targets, expected) {
            let chunks = range.get_required_chunks(target);
            let chunks = chunks
                .maybe_chunks
                .expect("range lens output should resolve");
            let mut actual = Vec::new();
            for chunk in chunks.iter() {
                let values = chunk.components().get_array(target).unwrap();
                actual.extend(
                    std::iter::zip(chunk.iter_indices(&TimelineName::log_tick()), values.iter())
                        .map(|((time, _row_id), value)| (time, value.unwrap().to_data())),
                );
            }
            actual.sort_by_key(|(index, _)| *index);
            assert_eq!(
                actual,
                std::iter::zip(rows, expected)
                    .map(|((tick, _row_id), value)| (TimeInt::new_temporal(tick), value.to_data()))
                    .collect::<Vec<_>>()
            );
        }
    });
}
