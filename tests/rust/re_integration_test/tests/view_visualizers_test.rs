//! Tests for the visualizers section in the selection panel when a view is selected.

use std::f64::consts::TAU;
use std::sync::Arc;

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::external::arrow::array::Float64Array;
use re_sdk::log::RowId;
use re_test_context::VisualizerBlueprintContext as _;
use re_viewer::external::re_log_types::EntityPath;
use re_viewer::external::re_sdk_types::{self, VisualizableArchetype as _, archetypes};
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

/// Test that shows the visualizers section in the selection panel when a view is selected.
///
/// This test:
/// 1. Logs time series data with `SeriesLines` and `SeriesPoints` styling
/// 2. Creates two `TimeSeriesViews`
/// 3. Selects a view and takes snapshots showing the visualizers list
#[tokio::test(flavor = "multi_thread")]
pub async fn test_view_visualizers_section() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1200.0, 1000.0)),
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(false);

    let timeline = re_sdk::Timeline::new_sequence("frame");

    // Log SeriesLines data (sine wave)
    harness.log_entity("plots/sin_line", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::SeriesLines::new()
                .with_colors([[255, 0, 0]])
                .with_names(["Sine Line"])
                .with_widths([2.0]),
        )
    });
    for i in 0..50 {
        harness.log_entity("plots/sin_line", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single((i as f64 / 50.0 * TAU).sin()),
            )
        });
    }

    // Log SeriesPoints data (cosine wave as scatter plot)
    harness.log_entity("plots/cos_points", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::SeriesPoints::new()
                .with_colors([[0, 255, 0]])
                .with_names(["Cosine Points"])
                .with_marker_sizes([4.0]),
        )
    });
    for i in 0..50 {
        harness.log_entity("plots/cos_points", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single((i as f64 / 50.0 * TAU).cos()),
            )
        });
    }

    // Log another SeriesLines entity for the second view
    harness.log_entity("other/linear", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::SeriesLines::new()
                .with_colors([[0, 0, 255]])
                .with_names(["Linear"])
                .with_widths([3.0]),
        )
    });
    for i in 0..50 {
        harness.log_entity("other/linear", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single(i as f64 / 50.0),
            )
        });
    }

    // Set up two TimeSeriesViews
    harness.clear_current_blueprint();

    let mut view_1 = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView::new_subtree("/plots"),
    );
    view_1.display_name = Some("Plots view".into());

    let mut view_2 = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView::new_subtree("/other"),
    );
    view_2.display_name = Some("Other view".into());

    harness.setup_viewport_blueprint(move |ctx, blueprint| {
        blueprint.add_views([view_1.clone(), view_2].into_iter(), None, None);

        // Add both SeriesLines AND SeriesPoints visualizers to the sin_line entity
        // This demonstrates that one entity can have multiple visualizers
        // Also demonstrates name overrides - the SeriesPoints visualizer has an overridden name
        ctx.save_visualizers(
            &EntityPath::from("plots/sin_line"),
            view_1.id,
            [
                archetypes::SeriesLines::new()
                    .with_colors([[255, 0, 0]])
                    .with_widths([2.0])
                    .visualizer(),
                archetypes::SeriesPoints::new()
                    .with_colors([[255, 100, 100]])
                    .with_names(["Sine Points (Override)"])
                    .with_marker_sizes([3.0])
                    .visualizer(),
            ],
        );

        // Override the name for the cos_points entity
        ctx.save_visualizers(
            &EntityPath::from("plots/cos_points"),
            view_1.id,
            [archetypes::SeriesPoints::new()
                .with_colors([[0, 255, 0]])
                .with_names(["Cosine (Override)"])
                .with_marker_sizes([4.0])
                .visualizer()],
        );
    });

    // Expand the blueprint tree to see the views
    harness
        .blueprint_tree()
        .right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");
    harness.run();

    // Snapshot 1: Initial state with two views
    harness.snapshot_app("view_visualizers_1_initial");

    // Select the first view (Plots view) - shows SeriesLines and SeriesPoints visualizers
    harness.blueprint_tree().click_label("Plots view");
    harness.run();

    // Snapshot 2: Selection panel showing visualizers for Plots view
    harness.snapshot_app("view_visualizers_2_plots_view_selected");

    // Select the second view (Other view) - shows only SeriesLines visualizer
    harness.blueprint_tree().click_label("Other view");
    harness.run();

    // Snapshot 3: Selection panel showing visualizers for Other view
    harness.snapshot_app("view_visualizers_3_other_view_selected");

    // --- Test removing visualizers ---

    // Select the Plots view again to see its 3 visualizers
    harness.blueprint_tree().click_label("Plots view");
    harness.run();

    // Click the first "Remove visualizer" trash button (removes sin_line's first visualizer)
    harness
        .selection_panel()
        .click_nth_label("Remove visualizer", 0);
    harness.run();

    // Snapshot 4: After removing the first visualizer from sin_line
    harness.snapshot_app("view_visualizers_4_after_remove_first");

    // Click the first trash button again (now removes whatever is first in the remaining list)
    harness
        .selection_panel()
        .click_nth_label("Remove visualizer", 0);
    harness.run();

    // Snapshot 5: After removing another visualizer
    harness.snapshot_app("view_visualizers_5_after_remove_second");
}

/// Test that shows the visualizer list when an entity logs multiple scalars per timestamp.
///
/// This test:
/// 1. Logs time series data with 3 scalar values per row (e.g. `Scalars::new([a, b, c])`)
/// 2. Creates a `TimeSeriesView` and selects it
/// 3. Takes a snapshot to inspect the visualizer list
#[tokio::test(flavor = "multi_thread")]
pub async fn test_view_visualizers_multi_scalar() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1200.0, 1000.0)),
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(false);

    let timeline = re_sdk::Timeline::new_sequence("frame");

    // Log an entity with 3 scalar values per timestamp
    for i in 0..50 {
        let t = i as f64 / 50.0;
        harness.log_entity("multi/triple", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::new([
                    (t * TAU).sin(),
                    (t * TAU).cos(),
                    t * 2.0 - 1.0,
                ]),
            )
        });
    }

    // Log an entity with 2 scalar values per timestamp
    for i in 0..50 {
        let t = i as f64 / 50.0;
        harness.log_entity("multi/double", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::new([(t * TAU).sin(), (t * TAU).cos()]),
            )
        });
    }

    // Log a single-scalar entity for comparison
    for i in 0..50 {
        let t = i as f64 / 50.0;
        harness.log_entity("multi/single", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single(t),
            )
        });
    }

    // Set up a TimeSeriesView
    harness.clear_current_blueprint();

    let mut view = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView::new_subtree("/multi"),
    );
    view.display_name = Some("Multi-scalar view".into());

    harness.setup_viewport_blueprint(move |_ctx, blueprint| {
        blueprint.add_views(std::iter::once(view.clone()), None, None);
    });

    // Expand the blueprint tree
    harness
        .blueprint_tree()
        .right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");
    harness.run();

    // Select the view to see its visualizers in the selection panel
    harness.blueprint_tree().click_label("Multi-scalar view");
    harness.run();

    // Snapshot: Selection panel showing visualizers for a multi-scalar entity
    harness.snapshot_app("view_visualizers_multi_scalar_view_selected");
}

/// Test the "+" button on the Visualizers section when a view is selected.
///
/// This test:
/// 1. Logs time series data
/// 2. Creates a `TimeSeriesView` and selects it
/// 3. Removes a visualizer so that the "+" popup has an option to offer
/// 4. Opens the popup, adds the visualizer back, verifies it reappears
#[tokio::test(flavor = "multi_thread")]
pub async fn test_view_visualizers_add_button() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1200.0, 1000.0)),
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(false);

    let timeline = re_sdk::Timeline::new_sequence("frame");

    // Log a sine wave
    harness.log_entity("trig/sin", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::SeriesLines::new()
                .with_colors([[255, 0, 0]])
                .with_names(["Sine"]),
        )
    });
    for i in 0..100 {
        harness.log_entity("trig/sin", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single((i as f64 / 100.0 * TAU).sin()),
            )
        });
    }

    // Log a cosine wave
    harness.log_entity("trig/cos", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::SeriesLines::new()
                .with_colors([[0, 128, 0]])
                .with_names(["Cosine"]),
        )
    });
    for i in 0..100 {
        harness.log_entity("trig/cos", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single((i as f64 / 100.0 * TAU).cos()),
            )
        });
    }

    // Set up a single TimeSeriesView rooted at /trig
    harness.clear_current_blueprint();
    let mut view = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView::new_subtree("/trig"),
    );
    view.display_name = Some("Trig view".into());

    harness.setup_viewport_blueprint(move |_ctx, blueprint| {
        blueprint.add_views(std::iter::once(view), None, None);
    });

    // Expand the blueprint tree
    harness
        .blueprint_tree()
        .right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");

    // Select the view
    harness.blueprint_tree().click_label("Trig view");

    // Snapshot 1: View selected — all entities already have visualizers, so
    // the "+" button is disabled (nothing new to add).
    harness.snapshot_app("view_visualizers_add_1_view_selected");

    // Remove the first visualizer so the popup has something to offer.
    harness
        .selection_panel()
        .click_nth_label("Remove visualizer", 0);

    // Click the "+" button to open the popup
    harness.selection_panel().click_label("Add new visualizer…");

    // Snapshot 2: Popup showing the removed entity's component as an option
    harness.snapshot_app("view_visualizers_add_2_popup_open");

    // Click the option to add the visualizer back.
    harness.click_nth_label("Scalars:scalars", 0);

    // Snapshot 3: After adding, the visualizer should reappear in the list
    harness.snapshot_app("view_visualizers_add_3_visualizer_added");
}

/// Test adding multiple new visualizers to a view via the "+" button, using custom message formats.
///
/// This test:
/// 1. Logs time series data for three entities using `DynamicArchetype` with custom component names
/// 2. Creates a `TimeSeriesView` and selects it
/// 3. Removes two visualizers so the popup has options to offer
/// 4. Opens the popup — custom components should appear grouped under their respective entities
/// 5. Adds them back one at a time, verifying the list grows with each addition
#[tokio::test(flavor = "multi_thread")]
pub async fn test_view_visualizers_add_multiple() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1200.0, 1000.0)),
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(false);

    let timeline = re_sdk::Timeline::new_sequence("frame");

    // Log three entities with custom (non-standard) component formats via DynamicArchetype.
    // Each entity has multiple fields so the Add New Visualizer popup displays
    // multiple custom components grouped under their respective entity paths.
    // We also log static SeriesLines colors per entity for deterministic plot rendering.
    harness.log_entity("waves/sin", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::SeriesLines::new().with_colors([[255, 0, 0]]),
        )
    });
    for i in 0..20 {
        let t = i as f64 / 20.0;
        harness.log_entity("waves/sin", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &re_sdk_types::DynamicArchetype::new("wave_data")
                    .with_component_from_data(
                        "amplitude",
                        Arc::new(Float64Array::from(vec![(t * TAU).sin()])),
                    )
                    .with_component_from_data("phase", Arc::new(Float64Array::from(vec![t * TAU]))),
            )
        });
    }

    harness.log_entity("waves/cos", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::SeriesLines::new().with_colors([[0, 128, 0]]),
        )
    });
    for i in 0..20 {
        let t = i as f64 / 20.0;
        harness.log_entity("waves/cos", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &re_sdk_types::DynamicArchetype::new("wave_data")
                    .with_component_from_data(
                        "signal",
                        Arc::new(Float64Array::from(vec![(t * TAU).cos()])),
                    )
                    .with_component_from_data(
                        "frequency",
                        Arc::new(Float64Array::from(vec![t * 10.0])),
                    ),
            )
        });
    }

    harness.log_entity("waves/ramp", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::SeriesLines::new().with_colors([[0, 0, 255]]),
        )
    });
    for i in 0..20 {
        let t = i as f64 / 20.0;
        harness.log_entity("waves/ramp", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &re_sdk_types::DynamicArchetype::new("wave_data")
                    .with_component_from_data("voltage", Arc::new(Float64Array::from(vec![t])))
                    .with_component_from_data(
                        "current",
                        Arc::new(Float64Array::from(vec![t * 0.5])),
                    ),
            )
        });
    }

    // Set up a single TimeSeriesView rooted at /waves
    harness.clear_current_blueprint();
    let mut view = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView::new_subtree("/waves"),
    );
    view.display_name = Some("Waves view".into());

    harness.setup_viewport_blueprint(move |_ctx, blueprint| {
        blueprint.add_views(std::iter::once(view), None, None);
    });

    // Expand the blueprint tree
    harness
        .blueprint_tree()
        .right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");

    // Select the view
    harness.blueprint_tree().click_label("Waves view");

    // Remove the first two visualizers so the popup has options to offer.
    // With entities sorted alphabetically, these are both from waves/cos.
    harness
        .selection_panel()
        .click_nth_label("Remove visualizer", 0);
    harness
        .selection_panel()
        .click_nth_label("Remove visualizer", 0);

    // Snapshot 1: View selected, two visualizers removed
    harness.snapshot_app("view_visualizers_add_multi_1_view_selected");

    // Open the add-visualizer popup
    harness.selection_panel().click_label("Add new visualizer…");

    // Snapshot 2: Popup showing custom components under their respective entity.
    // Both removed fields ("frequency" and "signal") appear under waves/cos.
    harness.snapshot_app("view_visualizers_add_multi_2_popup_open");

    // Add back one custom component (waves/cos → frequency)
    harness.click_label("wave_data:frequency");

    // Snapshot 3: First visualizer re-added to the list
    harness.snapshot_app("view_visualizers_add_multi_3_first_added");

    // Open the popup again and add the second custom component
    harness.selection_panel().click_label("Add new visualizer…");

    // Only one option remains (waves/cos → signal)
    harness.click_label("wave_data:signal");

    // Snapshot 4: Second visualizer also re-added to the list
    harness.snapshot_app("view_visualizers_add_multi_4_second_added");
}
