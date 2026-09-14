use re_integration_test::{HarnessConfig, InspectionHarness};

/// A viewer under test must never initialize analytics.
///
/// Only the in-process target runs the viewer in this process, so that is the target where this
/// assertion has teeth.
#[tokio::test(flavor = "multi_thread")]
pub async fn analytics_is_not_initialized_by_the_viewer() {
    let mut harness = InspectionHarness::spawn(HarnessConfig::default());
    harness.run_ok();

    assert!(
        !re_analytics::Analytics::global_init_was_attempted(),
        "The viewer tried to initialize analytics while running under test"
    );
}
