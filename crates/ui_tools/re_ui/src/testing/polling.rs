use super::default_snapshot_options_for_ui;

/// How long [`step_until`] sleeps between attempts by default.
pub const DEFAULT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);

/// How long [`step_until`] waits before timing out by default.
pub const DEFAULT_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Steps through the harness until the `predicate` closure returns `true`.
#[track_caller]
pub fn step_until<'app, 'harness, State, Predicate>(
    test_description: &'static str,
    harness: &'harness mut egui_kittest::Harness<'app, State>,
    predicate: Predicate,
) where
    Predicate: for<'a> FnMut(&'a mut egui_kittest::Harness<'app, State>) -> bool,
{
    step_until_with_custom_timeout(
        test_description,
        harness,
        predicate,
        DEFAULT_POLL_INTERVAL,
        DEFAULT_WAIT_TIMEOUT,
    );
}

/// Steps through the harness until the `predicate` closure returns `true`.
#[track_caller]
pub fn step_until_with_custom_timeout<'app, 'harness, State, Predicate>(
    test_description: &'static str,
    harness: &'harness mut egui_kittest::Harness<'app, State>,
    mut predicate: Predicate,
    poll_interval: std::time::Duration,
    timeout: std::time::Duration,
) where
    Predicate: for<'a> FnMut(&'a mut egui_kittest::Harness<'app, State>) -> bool,
{
    let start_time = std::time::Instant::now();
    let mut success = predicate(harness);
    while !success && start_time.elapsed() <= timeout {
        harness.step();
        std::thread::sleep(poll_interval);
        harness.step();
        success = predicate(harness);
    }

    if !success {
        // Take a screenshot of the state of the harness if we failed the test.
        // This is invaluable for debugging test failures.
        let snapshot_path = "tests/failures";
        harness
            .try_snapshot_options(
                test_description,
                &default_snapshot_options_for_ui().output_path(snapshot_path),
            )
            .ok();

        panic!(
            "Timed out waiting for predicate to be true for {test_description:?}. A screenshot of the harness has been saved to `{snapshot_path}/{test_description}.new.png`."
        );
    }
}
