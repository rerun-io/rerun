use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_types::{
    archetypes::{self, Scalars},
    blueprint, components,
};
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{ViewClass as _, ViewId, test_context::TestContext};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

#[test]
pub fn test_blueprint_overrides_and_defaults_with_time_series() {
    let mut test_context = get_test_context();

    for i in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), i)]);
        let t = i as f64 / 8.0;
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype(RowId::new(), timepoint.clone(), &Scalars::single(t.sin()))
        });
        test_context.log_entity("plots/cos", |builder| {
            builder.with_archetype(RowId::new(), timepoint, &Scalars::single(t.cos()))
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "blueprint_overrides_and_defaults_with_time_series",
        egui::vec2(300.0, 300.0),
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<TimeSeriesView>();

    test_context
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

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            test_context.run_with_single_view(ctx, view_id);
        });
    harness.run();
    harness.snapshot(name);
}
