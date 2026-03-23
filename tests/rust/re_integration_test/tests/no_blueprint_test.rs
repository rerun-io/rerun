// Test that the viewer works when no blueprint is sent from the SDK.
// This simulates the real SDK flow where only recording data arrives
// and the viewer must create a blueprint via heuristics.

use re_integration_test::HarnessExt as _;
use re_sdk::Timeline;
use re_viewer::external::re_sdk_types;
use re_viewer::viewer_test_utils::{self, AppTestingExt as _};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_no_blueprint_from_sdk() {
    let mut harness = viewer_test_utils::viewer_harness(&Default::default());
    harness.init_recording_without_blueprint();

    let timeline = Timeline::new_sequence("frame");

    // Log some Points3D data on a sequence timeline
    for frame in 0..5 {
        harness.log_entity("world/points", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, frame)],
                &re_sdk_types::archetypes::Points3D::new([(1.0, 2.0, 3.0)]),
            )
        });
    }

    // Let heuristics fire and the viewer settle
    harness.run_steps(5);

    // The recording's app should have an active blueprint registered.
    // Without the fix, no blueprint gets registered in `active_blueprint_by_app_id`
    // even though the viewer creates one as a fallback via `blueprint_entry`.
    let app = harness.state_mut();
    let route = app.testonly_get_route().clone();
    let app_id = route.app_id().expect("route should have an app_id").clone();
    let store_hub = app.testonly_get_store_hub();
    let has_active_blueprint = store_hub.active_blueprint_id_for_app(&app_id).is_some();
    assert!(
        has_active_blueprint,
        "Expected an active blueprint registered for app '{app_id}', but none was found"
    );

    harness.snapshot_app("no_blueprint_from_sdk");
}
