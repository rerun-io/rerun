use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{
    archetypes::{self, Scalars},
    blueprint, components,
    datatypes::{self, TimeRange},
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

    let view_id = setup_blueprint(&mut test_context, None);
    let size = egui::vec2(300.0, 300.0);
    test_context.run_view_ui_and_save_snapshot(
        view_id,
        "blueprint_overrides_and_defaults_with_time_series",
        size,
        None,
    );

    for (range, name) in [
        (TimeRange::EVERYTHING, "everything"),
        (TimeRange::AT_CURSOR, "at_cursor"),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::CursorRelative(datatypes::TimeInt(-10)),
                end: datatypes::TimeRangeBoundary::CursorRelative(datatypes::TimeInt(10)),
            },
            "around_cursor",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(10)),
                end: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(20)),
            },
            "absolute",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(10)),
                end: datatypes::TimeRangeBoundary::Infinite,
            },
            "absolute_until_end",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Infinite,
                end: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(15)),
            },
            "start_until_absolute",
        ),
    ] {
        let view_id = setup_blueprint(&mut test_context, Some(range));
        test_context.run_view_ui_and_save_snapshot(
            view_id,
            &format!("blueprint_overrides_and_defaults_with_time_series_{name}"),
            size,
            None,
        );
    }
}

fn setup_blueprint(test_context: &mut TestContext, time_axis_view: Option<TimeRange>) -> ViewId {
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

        if let Some(time_axis_view) = time_axis_view {
            let time_axis = re_viewport_blueprint::ViewProperty::from_archetype::<
                blueprint::archetypes::TimeAxis,
            >(ctx.blueprint_db(), ctx.blueprint_query, view.id);

            time_axis.save_blueprint_component(
                ctx,
                &blueprint::archetypes::TimeAxis::descriptor_view_range(),
                &time_axis_view,
            );
        }

        blueprint.add_view_at_root(view)
    })
}
