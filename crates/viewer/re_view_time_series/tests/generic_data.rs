//! Tests for visualizing generic (non-Rerun) data types.

use std::sync::Arc;

use re_log_types::Timeline;
use re_log_types::external::arrow::array::{Int32Array, StructArray};
use re_log_types::external::arrow::datatypes::{DataType, Field};
use re_sdk_types::DynamicArchetype;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

const MAX_TIME: i64 = 31;

/// Tests that a nested Int32 array (inside a struct) can be visualized in a time series view.
///
/// Visualizing nested structs of plot-able data should not require any extra setup via blueprints.
#[test]
pub fn test_nested_int32_to_scalar_cast() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<TimeSeriesView>();

    let timeline = Timeline::log_tick();

    for i in 0..=MAX_TIME {
        let offsets = [-50, 0, 50];
        let int_values: Vec<i32> = offsets
            .iter()
            .map(|&offset| (i as i32 * 10) + offset)
            .collect();

        let struct_array = StructArray::from(vec![(
            Arc::new(Field::new("values", DataType::Int32, false)),
            Arc::new(Int32Array::from(int_values))
                as Arc<dyn re_log_types::external::arrow::array::Array>,
        )]);

        test_context.log_entity("data", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component_from_data("nested", Arc::new(struct_array)),
            )
        });
    }

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline(*timeline.name())],
    );

    let view_id = setup_blueprint(&mut test_context);

    let size = egui::vec2(300.0, 300.0);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "nested_int32_to_scalar_cast",
        size,
        None,
    ));
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());
        blueprint.add_view_at_root(view)
    })
}
