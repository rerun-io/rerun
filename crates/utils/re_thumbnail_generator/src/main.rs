use egui::accesskit::Role;
use re_data_source::{DataSource, StreamSource};
use re_entity_db::StoreBundle;
use re_log_encoding::VersionPolicy;
use re_log_types::external::re_types_core::Archetype;
use re_log_types::{EntityPath, FileSource, LogMsg, StoreKind, TimeReal, Timeline};
use re_smart_channel::{SmartMessage, SmartMessagePayload};
use re_view_spatial::{SpatialView2D, SpatialView3D};
use re_viewer_context::external::egui_kittest::kittest::Queryable;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass, ViewId};
use re_viewport::external::re_types;
use re_viewport_blueprint::test_context_ext::TestContextExt;
use re_viewport_blueprint::ViewBlueprint;
use std::thread;
use std::time::Duration;

type ViewType = SpatialView3D;

fn main() {
    let input_path = std::env::args().nth(1).expect("No path provided");
    let output_path = std::env::args().nth(2).expect("No output path provided");

    let mut context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    context.register_view_class::<ViewType>();

    let file = std::fs::File::open(input_path).expect("Failed to open file");

    let mut bundle =
        StoreBundle::from_rrd(VersionPolicy::Error, file).expect("Failed to load bundle");

    context.recording_store = bundle
        .drain_entity_dbs()
        .find(|db| db.store_id().kind == StoreKind::Recording)
        .expect("No recording found");

    context
        .recording_config
        .time_ctrl
        .write()
        .set_timeline(Timeline::new_temporal("timestamp"));

    //CAUTION: that must be AFTER the timeline is set
    let view_id = setup_blueprint(&mut context);

    let mut harness = context.setup_kittest_for_rendering().build(|ctx| {
        re_ui::apply_style_and_install_loaders(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            context.run(ctx, |ctx| {
                let view_class = ctx
                    .view_class_registry()
                    .get_class_or_log_error(ViewType::identifier());

                let view_blueprint = ViewBlueprint::try_from_db(
                    view_id,
                    ctx.store_context.blueprint,
                    ctx.blueprint_query,
                )
                .expect("we just created that view");

                let mut view_states = context.view_states.lock();
                let view_state = view_states.get_mut_or_create(view_id, view_class);

                let (view_query, system_execution_output) =
                    re_viewport::execute_systems_for_view(ctx, &view_blueprint, view_state);

                view_class
                    .ui(ctx, ui, view_state, &view_query, system_execution_output)
                    .expect("failed to run graph view ui");
            });

            context.handle_system_commands();
        });
    });

    // Center the view by simulating double clicks
    harness.get_by_role(Role::Unknown).simulate_click();
    harness.step();
    harness.get_by_role(Role::Unknown).simulate_click();
    harness.step();

    let image = harness.render().expect("Failed to render");

    image.save(output_path).expect("Failed to save image");

    println!("Done!");
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            ViewType::identifier(),
            RecommendedView::new_single_entity("vehicle_local_position/pos".into()),
        );

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        let visible_time_range_list = vec![re_types::blueprint::components::VisibleTimeRange(
            re_types::datatypes::VisibleTimeRange {
                timeline: "timestamp".into(),
                range: re_types::datatypes::TimeRange {
                    start: re_types::datatypes::TimeRangeBoundary::Infinite,
                    end: re_types::datatypes::TimeRangeBoundary::Infinite,
                },
            },
        )];
        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view_id,
            ctx.store_context.blueprint.tree(),
            re_types::blueprint::archetypes::VisibleTimeRanges::name(),
        );

        ctx.save_blueprint_component(&property_path, &visible_time_range_list);

        let radius_override_entity_path = view_id.as_entity_path().join(&EntityPath::from(
            "ViewContents/individual_overrides/vehicle_local_position/pos",
        ));
        ctx.save_blueprint_component(
            &radius_override_entity_path,
            &[re_types::components::Radius::new_ui_points(10.0)],
        );

        view_id
    })
}
