//! Test that the spawn limit is respected when auto-spawning views.
//!
//! Uses time series views for this.

use std::sync::Arc;

use re_integration_test::HarnessExt as _;
use re_sdk::Timeline;
use re_sdk::external::arrow::array::Float64Array;
use re_view_time_series::TimeSeriesView;
use re_viewer::external::re_sdk_types;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::viewer_test_utils;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_time_series_max_views_spawned() {
    let mut harness = viewer_test_utils::viewer_harness(&Default::default());
    harness.init_recording();

    let timeline = Timeline::new_sequence("frame");

    // Log 12 different scalar entities (exceeding the default limit of 8)
    // Mix native Scalars and custom Float64 components to test both paths

    for frame in 0..10 {
        // Native Scalars (entities 0-5)
        for i in 0..6 {
            for frame in 0..10 {
                harness.log_entity(format!("native_{i}"), |builder| {
                    builder.with_archetype_auto_row(
                        [(timeline, frame)],
                        &re_sdk_types::archetypes::Scalars::single(frame as f64 * i as f64),
                    )
                });
            }
            // Custom Float64 components (entities 6-11)
            for i in 6..12 {
                harness.log_entity(format!("custom_{i}"), |builder| {
                    builder.with_archetype_auto_row(
                        [(timeline, frame)],
                        &re_sdk_types::DynamicArchetype::new("custom").with_component_from_data(
                            "custom_scalar",
                            Arc::new(Float64Array::from(vec![frame as f64 * i as f64])),
                        ),
                    )
                });
            }
        }
    }

    // Explicitly enable auto views.
    harness.setup_viewport_blueprint(|ctx, blueprint| {
        blueprint.set_auto_layout(true, ctx);
        blueprint.set_auto_views(true, ctx);
    });

    // Collect all spawned TimeSeriesViews with their entity path filters
    let mut time_series_view_descriptions = harness.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint
            .views
            .values()
            .filter(|view| view.class_identifier() == TimeSeriesView::identifier())
            .map(|view| {
                format!(
                    "origin: {}, filter: {:?}",
                    view.space_origin,
                    view.contents.entity_path_filter()
                )
            })
            .collect::<Vec<_>>()
    });

    // We've iterated a hashmap, so have to sort.
    time_series_view_descriptions.sort();

    // We logged 12 entities, but the default max is 8, so we should have at most 8 views
    assert_eq!(
        time_series_view_descriptions.len(),
        8,
        "Expected exactly 8 TimeSeriesViews to be spawned (respecting DEFAULT_MAX_VIEWS_SPAWNED)"
    );
    insta::assert_snapshot!(time_series_view_descriptions.join("\n"), @r#"
    origin: /custom_6, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    origin: /custom_7, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    origin: /native_0, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    origin: /native_1, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    origin: /native_2, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    origin: /native_3, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    origin: /native_4, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    origin: /native_5, filter: ResolvedEntityPathFilter("+ $origin\n- /__properties/**")
    "#);
}
