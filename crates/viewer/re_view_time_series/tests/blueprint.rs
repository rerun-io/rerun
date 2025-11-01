use std::sync::Arc;

use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{
    Archetype as _, DynamicArchetype,
    archetypes::{self, Scalars},
    blueprint, components,
    external::arrow::array::{Float32Array, Int64Array},
};
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

#[test]
pub fn test_blueprint_overrides_and_defaults_with_time_series() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let timeline = re_log_types::Timeline::log_tick();

    for i in 0..32 {
        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype(RowId::new(), timepoint.clone(), &Scalars::single(t.sin()))
        });
        test_context.log_entity("plots/cos", |builder| {
            builder.with_archetype(RowId::new(), timepoint, &Scalars::single(t.cos()))
        });
    }

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline(*timeline.name())],
    );

    let view_id = setup_blueprint(&mut test_context);
    test_context.run_view_ui_and_save_snapshot(
        view_id,
        "blueprint_overrides_and_defaults_with_time_series",
        egui::vec2(300.0, 300.0),
        None,
    );
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        // Overrides:
        let cos_override_path =
            ViewContents::override_path_for_entity(view.id, &EntityPath::from("plots/cos"));
        ctx.save_blueprint_archetype(
            cos_override_path.clone(),
            // Override which visualizer to use for the `cos` plot.
            &blueprint::archetypes::VisualizerOverrides::new(["SeriesPoints"]),
        );
        ctx.save_blueprint_archetype(
            cos_override_path,
            // Override color and markers for the `cos` plot.
            &archetypes::SeriesPoints::default()
                .with_colors([(0, 255, 0)])
                .with_markers([components::MarkerShape::Cross]),
        );

        // Override default color (should apply to the `sin` plot).
        ctx.save_blueprint_archetype(
            view.defaults_path.clone(),
            &archetypes::SeriesLines::default().with_colors([(0, 0, 255)]),
        );

        blueprint.add_view_at_root(view)
    })
}

// TODO: Move this test to a better place.
#[test]
pub fn test_blueprint_f64_with_time_series() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let timeline = re_log_types::Timeline::log_tick();

    for i in 0..32 {
        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype(RowId::new(), timepoint.clone(), &Scalars::single(t.sin()))
        });
        test_context.log_entity("plots/cos", |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint.clone(),
                // an untagged component
                &DynamicArchetype::new(Scalars::name()).with_component_from_data(
                    "scalars",
                    Arc::new(Float32Array::from(vec![t.cos() as f32])),
                ),
            )
        });
        test_context.log_entity("plots/line", |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                // an untagged component
                &DynamicArchetype::new(Scalars::name()).with_component_from_data(
                    "scalars",
                    // Something that stays in the same domain as a sine wave.
                    Arc::new(Int64Array::from(vec![(i % 2) * 2 - 1])),
                ),
            )
        });
    }

    // test_context
    //     .save_recording_to_file("/Users/goertler/Desktop/dyn_f64.rrd")
    //     .unwrap();

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline(*timeline.name())],
    );

    let view_id = setup_descriptor_override_blueprint(&mut test_context);
    test_context.run_view_ui_and_save_snapshot(
        view_id,
        "blueprint_f64_with_time_series",
        egui::vec2(300.0, 300.0),
        None,
    );
}

fn setup_descriptor_override_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        blueprint.add_view_at_root(view)
    })
}
