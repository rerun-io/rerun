//! Integration test: load example .rrd files from the previous release into the current viewer.
//!
//! This catches backward-compatibility regressions for both recording data and blueprints.
//! The previous release version is derived from the workspace `CARGO_PKG_VERSION`.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{SnapshotOptions, SnapshotResults};
use futures::StreamExt as _;
use re_integration_test::HarnessExt as _;
use re_viewer::external::re_ui::notifications::NotificationLevel;
use re_viewer::viewer_test_utils::{self, AppTestingExt as _, HarnessOptions, step_until};
use re_viewer::{SystemCommand, SystemCommandSender as _};
use re_viewer_context::TimeControlCommand;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Maximum number of concurrent downloads.
const DOWNLOAD_CONCURRENCY: usize = 8;

/// Derive previous minor version from `CARGO_PKG_VERSION`.
///
/// E.g., `"0.32.0-alpha.1+dev"` → `(0, 31)`.
fn previous_minor_version() -> (u32, u32) {
    let version = env!("CARGO_PKG_VERSION"); // e.g. "0.32.0-alpha.1+dev"
    let parts: Vec<&str> = version.split('.').collect();
    let major: u32 = parts[0].parse().expect("failed to parse major version");
    let minor: u32 = parts[1].parse().expect("failed to parse minor version");
    assert!(
        minor > 0,
        "Cannot derive previous version from minor=0 (version={version})"
    );
    (major, minor - 1)
}

/// Probe `app.rerun.io` to find the latest patch for a given `major.minor`.
///
/// Tries `major.minor.0`, `major.minor.1`, … until a HEAD request returns 404.
async fn resolve_latest_patch(client: &reqwest::Client, major: u32, minor: u32) -> String {
    let mut patch = 0u32;
    loop {
        let next = patch + 1;
        let version = format!("{major}.{minor}.{next}");
        let url = format!("https://app.rerun.io/version/{version}/examples/plots.rrd");
        match client.head(&url).send().await {
            Ok(resp) if resp.status().is_success() => patch = next,
            _ => break,
        }
    }
    format!("{major}.{minor}.{patch}")
}

/// Notification messages that are expected to be triggered by at least one example.
///
/// The test fails if any of these is never triggered, so that entries are removed
/// from this list once the underlying issue is fixed.
const EXPECTED_NOTIFICATIONS: &[&str] = &[];

/// Examples whose heuristic-generated blueprints art unstable in some way.
///
/// For these, "Reset blueprint" will be called once after everything loads.
const UNSTABLE_BLUEPRINT_EXAMPLES: &[&str] = &[
    // `segmentation/rgb_scaled` vs `segmentation` have different image sizes;
    // depending on arrival order the heuristic either splits them into two views
    // or groups them into one.
    "detect_and_track_objects",
];

/// Examples which are completely nondeterministic, snapshots will be skipped.
const NONDETERMINISTIC_EXAMPLES: &[&str] = &[
    // The graphs are physics based and vary every reload
    "graphs",
];

/// Examples that contain a `MapView` whose OSM tiles can change as OSM updates.
///
/// We mask the map view's pane so the snapshot stays stable.
const MAP_VIEW_EXAMPLES: &[&str] = &[
    // Uses `rrb.MapView(name="MapView", …)`.
    "nuscenes_dataset",
];

/// Examples whose snapshots are unstable enough on macOS/Windows that we need to
/// bump `failed_pixel_count_threshold` on those platforms to avoid spurious CI failures.
const HIGH_THRESHOLD_TESTS: &[&str] = &[
    // Small but consistent rendering diff on macOS.
    "rgbd",
    // Photogrammetry mesh rendering diverges noticeably on macOS.
    "open_photogrammetry_format",
    // The transparent gripper is slightly flakey
    "animated_urdf",
];

/// An entry from the examples manifest hosted at `app.rerun.io`.
#[derive(serde::Deserialize)]
struct ManifestEntry {
    name: String,
    rrd_url: String,
}

/// Fetch the example manifest for a given version from `app.rerun.io`.
///
/// This returns only the stable examples shown on `rerun.io/viewer`.
async fn fetch_example_manifest(client: &reqwest::Client, version: &str) -> Vec<ManifestEntry> {
    let url = format!("https://app.rerun.io/version/{version}/examples_manifest.json");
    let resp = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .unwrap_or_else(|e| panic!("Failed to fetch example manifest at {url}: {e}"));
    resp.json()
        .await
        .unwrap_or_else(|e| panic!("Failed to parse example manifest: {e}"))
}

/// Download a URL to a local path, streaming chunks to disk.
async fn download(client: &reqwest::Client, url: &str, path: &Path) {
    let mut resp = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .unwrap_or_else(|e| panic!("Failed to download {url}: {e}"));
    let mut file =
        std::fs::File::create(path).unwrap_or_else(|e| panic!("Failed to create {path:?}: {e}"));
    while let Some(chunk) = resp
        .chunk()
        .await
        .unwrap_or_else(|e| panic!("Failed to read from {url}: {e}"))
    {
        file.write_all(&chunk)
            .unwrap_or_else(|e| panic!("Failed to write {path:?}: {e}"));
    }
}

/// Ensure a single example is cached at `path`, downloading it if missing.
async fn ensure_rrd_cached(
    client: &reqwest::Client,
    entry: &ManifestEntry,
    path: &Path,
    version: &str,
) {
    if path.exists() {
        return;
    }
    eprintln!("Downloading {}.rrd ({version})…", entry.name);
    download(client, &entry.rrd_url, path).await;
}

/// Load example .rrd files from the previous release into the current viewer.
///
/// Asserts:
/// - No panics during load + render
/// - At least one .rrd was downloaded and loaded
/// - A snapshot is saved for each example (for visual review)
#[tokio::test(flavor = "multi_thread")]
async fn test_old_rrds_in_current_viewer() {
    let client = reqwest::Client::new();
    let (major, prev_minor) = previous_minor_version();
    let version = resolve_latest_patch(&client, major, prev_minor).await;
    eprintln!("Testing backward compatibility with version {version}");

    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../../target/rrd_bw_compat/{version}"));
    std::fs::create_dir_all(&cache_dir).expect("failed to create cache directory");

    let manifest = fetch_example_manifest(&client, &version).await;
    assert!(
        !manifest.is_empty(),
        "Should have at least one example in the manifest for version {version}"
    );

    // Buffer up to `DOWNLOAD_CONCURRENCY` downloads in flight; yield each path as
    // soon as its download finishes so the next test can start immediately.
    let mut downloads = futures::stream::iter(manifest.into_iter().map(|entry| {
        let path = cache_dir.join(format!("{}.rrd", entry.name));
        let version = version.clone();
        let client = client.clone();
        async move {
            ensure_rrd_cached(&client, &entry, &path, &version).await;
            path
        }
    }))
    .buffer_unordered(DOWNLOAD_CONCURRENCY);

    let mut results = SnapshotResults::new();
    let mut expected_triggered = vec![false; EXPECTED_NOTIFICATIONS.len()];

    while let Some(rrd_path) = downloads.next().await {
        let example_name = rrd_path.file_stem().unwrap().to_str().unwrap().to_owned();
        eprintln!("Loading {example_name}.rrd…");

        // Open the .rrd via the viewer's normal file-open path (same as Cmd+O).
        let file_path = rrd_path.canonicalize().unwrap().display().to_string();
        let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
            window_size: Some(egui::vec2(1024.0, 768.0)),
            startup_url: Some(file_path),
            max_steps: Some(200),
            ..Default::default()
        });

        // Wait for the loading popup to disappear.
        step_until(
            "loading popup dismissed",
            &mut harness,
            |harness| {
                !harness
                    .query_all_by_role(Role::Window)
                    .any(|window| window.query_by_label_contains("Loading").is_some())
            },
            Duration::from_millis(100),
            Duration::from_secs(10),
        );

        assert!(
            harness.state().active_recording_id().is_some(),
            "{example_name}.rrd did not produce a recording."
        );

        // Pause playback and seek to the end of the recording so the snapshot
        // is deterministic and the viewer stops requesting repaints.
        harness.run_with_viewer_context(|ctx| {
            ctx.send_time_commands(vec![TimeControlCommand::Pause, TimeControlCommand::MoveEnd]);
        });
        harness.run();

        if UNSTABLE_BLUEPRINT_EXAMPLES.contains(&example_name.as_str()) {
            harness.run_with_viewer_context(|ctx| {
                ctx.command_sender()
                    .send_system(SystemCommand::ClearActiveBlueprintAndEnableHeuristics);
            });
            harness.run();
        }

        // Close all panels so the snapshot only shows the viewport.
        harness.set_blueprint_panel_opened(false);
        harness.set_selection_panel_opened(false);
        harness.set_time_panel_opened(false);

        // Mask OSM-tile-backed map views whose content may change as OSM updates.
        if MAP_VIEW_EXAMPLES.contains(&example_name.as_str()) {
            let map_rect = harness.get_by_role_and_label(Role::Pane, "MapView").rect();
            harness.mask(map_rect);
        }

        if !NONDETERMINISTIC_EXAMPLES.contains(&example_name.as_str()) {
            // Mask any timestamp text so snapshots stay stable as the calendar
            // day rolls over.
            harness.mask_dates();

            let snapshot_options = if HIGH_THRESHOLD_TESTS.contains(&example_name.as_str()) {
                SnapshotOptions::new()
                    .threshold(2.0)
                    .failed_pixel_count_threshold(10_000)
            } else {
                SnapshotOptions::new()
                    .threshold(2.0)
                    .failed_pixel_count_threshold(50)
            };
            harness.snapshot_options(format!("rrd_bw_compat_{example_name}"), &snapshot_options);
        }

        // Assert no unexpected warnings or errors were shown to the user, and
        // record which expected notifications were triggered.
        let mut bad_notifications = vec![];
        for n in harness
            .state()
            .testonly_get_notifications()
            .notifications()
            .iter()
            .filter(|n| {
                matches!(
                    n.level(),
                    NotificationLevel::Warning | NotificationLevel::Error
                )
            })
        {
            let mut matched = false;
            for (i, expected) in EXPECTED_NOTIFICATIONS.iter().enumerate() {
                if n.text().contains(expected) {
                    expected_triggered[i] = true;
                    matched = true;
                    break;
                }
            }
            if !matched {
                bad_notifications.push(format!("[{:?}] {}", n.level(), n.text()));
            }
        }
        assert!(
            bad_notifications.is_empty(),
            "{example_name}.rrd produced unexpected notifications:\n{}",
            bad_notifications.join("\n")
        );

        results.extend_harness(&mut harness);
    }

    let untriggered: Vec<&str> = EXPECTED_NOTIFICATIONS
        .iter()
        .zip(expected_triggered.iter())
        .filter(|(_, triggered)| !**triggered)
        .map(|(msg, _)| *msg)
        .collect();
    assert!(
        untriggered.is_empty(),
        "Expected notifications were not triggered by any example (remove them from EXPECTED_NOTIFICATIONS):\n{}",
        untriggered.join("\n")
    );
}
