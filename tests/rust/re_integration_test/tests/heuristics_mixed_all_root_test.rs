use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_sdk_types;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

fn make_multi_view_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();

    // Log some data
    harness.log_entity("boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
                [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF]),
        )
    });
    harness.log_entity("boxes2d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Boxes2D::from_centers_and_half_sizes(
                [(-1.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(0.2, 0.4), (0.2, 0.2), (0.4, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF]),
        )
    });

    let timeline = re_sdk::Timeline::new_sequence("timeline_a");
    harness.log_entity("text_log", |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline, 1)],
            &re_sdk_types::archetypes::TextLog::new("Hello World!")
                .with_level(re_sdk_types::components::TextLogLevel::INFO),
        )
    });

    harness.log_entity("points2d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Points2D::new([(1.0, 1.0), (1.2, -0.3), (0.0, -0.3)])
                .with_radii([0.1, 0.1, 0.1])
                .with_colors([0xFF9001FF, 0x9001FFFF, 0x90FF01FF])
                .with_labels(["a", "b", "c"]),
        )
    });

    harness.log_entity("text_document", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::TextDocument::new("Hello World!"),
        )
    });

    harness.set_selection_panel_opened(false);
    harness
}

fn sort_views_by_class_identifier(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    let mut views = harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.views.values().cloned().collect::<Vec<_>>()
    });
    views.sort_by_key(|v| v.class_identifier());
    harness.clear_current_blueprint();
    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.add_views(views.into_iter(), None, None);
    });
}

// Tests whether blueprint heuristics work correctly when mixing 2D and 3D data.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_heuristics_mixed_all_root() {
    let mut harness = make_multi_view_test_harness();

    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.set_auto_layout(true, _viewer_context);
        blueprint.set_auto_views(true, _viewer_context);
    });

    // Views are in a random order, lets order them
    sort_views_by_class_identifier(&mut harness);

    harness.snapshot_app("heuristics_mixed_all_root");
}
