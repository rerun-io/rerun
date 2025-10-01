// #![cfg(feature = "testing")]

use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::{SnapshotOptions, kittest::Queryable as _};

use re_chunk::{RowId, TimePoint};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{Archetype as _, components::Colormap};
// use re_viewer::viewer_test_utils;
use re_viewer_context::{MaybeMutRef, ViewClass, ViewerContext};
use re_viewport::ViewportUi;
use re_viewport_blueprint::ViewBlueprint;

fn x() -> Vec<f32> {
    (0..100).map(|i| i as f32 * 100.0 / 99.0).collect()
}

/// Navigates from welcome to settings screen and snapshots it.
#[test]
fn blueprint_test_ponies() {
    let mut test_context = TestContext::new_with_view_class::<re_view_bar_chart::BarChartView>();

    test_context.log_entity("tensor", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_types::archetypes::BarChart::new(x()),
        )
    });

    let rbl_path = "temp42.rbl";

    let bp1 = ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier());
    let bp2 = ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier());
    let bp3 = ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier());
    let bp4 = ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier());
    let bp5 = ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier());

    let bp1_id = bp1.id;
    // let bp2_id = bp2.id;

    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let color_override = re_types::archetypes::BarChart::default().with_color([0, 155, 0]);
        let override_path = re_viewport_blueprint::ViewContents::override_path_for_entity(
            bp1.id,
            &re_chunk::EntityPath::from("tensor"),
        );
        ctx.save_blueprint_archetype(override_path.clone(), &color_override);

        blueprint.add_views([bp1, bp2].into_iter(), None, None);
    });

    save_blueprint_to_file(&test_context, rbl_path);

    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.remove_contents(re_viewer_context::Contents::View(bp1_id));
        blueprint.add_views([bp3, bp4, bp5].into_iter(), None, None);
    });

    load_blueprint_from_file(&mut test_context, rbl_path);

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(egui::vec2(600.0, 400.0))
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                let viewport_blueprint = re_viewport_blueprint::ViewportBlueprint::from_db(
                    ctx.blueprint_db(),
                    &test_context.blueprint_query,
                );
                let viewport_ui = ViewportUi::new(viewport_blueprint);
                viewport_ui.viewport_ui(ui, ctx, &mut test_context.view_states.lock());
            });

            test_context.handle_system_commands();
        });
    harness.run();
    harness.snapshot("xblueprint_test_ponies");
}

fn save_blueprint_to_file(test_context: &TestContext, path: &str) {
    test_context
        .save_blueprint_to_file(path)
        .expect("Failed to save blueprint to file.");
}

fn load_blueprint_from_file(test_context: &mut TestContext, path: &str) {
    let file = std::fs::File::open(path).expect("Failed to open blueprint file.");
    let rbl_store =
        re_entity_db::StoreBundle::from_rrd(file).expect("Failed to load blueprint store");
    {
        let mut lock = test_context.store_hub.lock();
        let app_id = lock.active_app().expect("Missing active app").clone();
        lock.try_to_load_persisted_blueprint_store(rbl_store, &app_id)
            .expect("Failed to load blueprint store");
    }

    // Trigger recalculation of visualizable entities and blueprint overrides.
    test_context.setup_viewport_blueprint(|_ctx, _blueprint| {});
}

fn take_snapshot(test_context: &mut TestContext, snapshot_name: &str) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(egui::vec2(600.0, 400.0))
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                let viewport_blueprint = re_viewport_blueprint::ViewportBlueprint::from_db(
                    ctx.blueprint_db(),
                    &test_context.blueprint_query,
                );
                let viewport_ui = ViewportUi::new(viewport_blueprint);
                viewport_ui.viewport_ui(ui, ctx, &mut test_context.view_states.lock());
            });

            test_context.handle_system_commands();
        });
    harness.run();
    harness.snapshot(snapshot_name);
}
