#![cfg(feature = "testing")]

use std::time::Duration;

use egui::accesskit::Role;
use egui::os::OperatingSystem;
use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_sdk_types::ColormapSelection;
use re_sdk_types::components::Colormap;
use re_test_context::TestContext;
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewer_context::MaybeMutRef;

fn os_snapshot_suffix(os: OperatingSystem) -> &'static str {
    match os {
        OperatingSystem::Nix => "linux",
        OperatingSystem::Mac => "mac",
        OperatingSystem::Windows => "windows",
        OperatingSystem::Unknown => "unknown",
        OperatingSystem::Android => "android",
        OperatingSystem::IOS => "ios",
    }
}

/// Navigates from welcome to settings screen and snapshots it.
#[tokio::test]
async fn settings_screen() {
    #![expect(unsafe_code)] // It's only a test

    // SAFETY: it's only a test
    unsafe {
        std::env::set_var("TZ", "Europe/Stockholm");
    }

    let mut snapshot_results = SnapshotResults::new();

    for os in [
        OperatingSystem::Nix,
        OperatingSystem::Mac,
        OperatingSystem::Windows,
    ] {
        let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
            window_size: Some(egui::vec2(1024.0, 1080.0)), // Settings screen can be a bit tall
            os: Some(os),
            ..Default::default()
        });
        harness.get_by_label("Menu").click();
        harness.run_ok();
        harness.get_by_label_contains("Settings…").click();
        // Wait for the FFmpeg-check loading indicator to disappear.
        viewer_test_utils::step_until(
            "Settings screen shows up with FFMpeg binary not found error",
            &mut harness,
            |harness| {
                harness
                    .query_by_label_contains(
                        "The specified FFmpeg binary path does not exist or is not a file.",
                    )
                    .is_some()
            },
            Duration::from_millis(100),
            Duration::from_secs(5),
        );
        snapshot_results
            .add(harness.try_snapshot(format!("settings_screen_{}", os_snapshot_suffix(os))));
    }
}

/// Snapshots the "About Rerun" menu content with a fixed, realistic `BuildInfo`.
#[test]
fn about_rerun() {
    let test_context = TestContext::new();

    let build_info = re_build_info::BuildInfo {
        crate_name: "rerun-cli".into(),
        features: "default analytics map_view nasm".into(),
        version: re_build_info::CrateVersion {
            major: 0,
            minor: 33,
            patch: 0,
            meta: None,
        },
        rustc_version: "1.84.0 (9fc6b4312 2025-01-07)".into(),
        llvm_version: "19.1.5".into(),
        git_hash: "abc1234deadbeefcafebabe00000000000000".into(),
        git_branch: "main".into(),
        is_in_rerun_workspace: true,
        target_triple: "aarch64-apple-darwin".into(),
        datetime: "2026-05-25T12:34:56Z".into(),
        is_debug_build: false,
    };

    let harness = test_context
        .setup_kittest_for_rendering_ui([460.0, 360.0])
        .with_theme(egui::Theme::Light);

    // let render_state = test_context.egui_render_state.lock().clone();
    let render_state = None; // Otherwise we get different results on different platforms

    let mut harness = harness.build_ui(|ui| {
        re_ui::apply_style_and_install_loaders(ui.ctx());
        egui::containers::menu::menu_style(ui.style_mut()); // The about-dialog is in a menu
        re_viewer::about_rerun_ui(ui, &build_info, render_state.as_ref());
    });

    harness.run();
    harness.fit_contents();
    harness.snapshot("about_rerun");
}

/// Opens the Rerun menu without an active recording and snapshots the app.
/// Tests that certain recording-related entries are disabled (e.g. save or close recording).
#[tokio::test]
async fn menu_without_recording() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.get_by_label("Menu").click();
    harness.run_ok();
    // Redact the shortcut for quitting as it's platform-dependent.
    harness.mask(harness.get_by_label_contains("Quit").rect());
    harness.snapshot("menu_without_recording");
}

/// Tests the colormap selector UI with snapshot testing.
/// This is defined here instead of in `re_viewer/tests` because it depends on `re_test_context`,
/// which depends on `re_viewer_context`.
#[test]
fn colormap_selector_ui() {
    let mut test_context = TestContext::new();
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    re_data_ui::register_component_uis(&mut test_context.component_ui_registry);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([200.0, 250.0])
        .build_ui(|ui| {
            re_ui::apply_style_and_install_loaders(ui.ctx());

            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                ui.horizontal(|ui| {
                    ui.label("Colormap:");

                    let mut test_colormap = Colormap::Spectral;
                    let mut colormap_ref = MaybeMutRef::MutRef(&mut test_colormap);

                    // Show the full selection of colormaps including the grid map category in this test.
                    re_viewer_context::gpu_bridge::colormap_edit_or_view_ui_with_selection(
                        ctx,
                        ui,
                        &mut colormap_ref,
                        ColormapSelection::IncludeGridMap,
                    );
                });
            });
        });

    harness.run();
    harness.fit_contents();
    harness.snapshot("colormap_selector_closed");

    // give the combo box some room to open
    harness.set_size(egui::Vec2::new(200.0, 400.0));
    harness.get_by_role(Role::ComboBox).click(); // open combo box
    harness.run();

    harness.fit_contents();
    harness.run();
    harness.snapshot("colormap_selector_open");
}

#[test]
fn ci_runners_use_software_rendering() {
    if std::env::var("CI").is_ok() {
        let test_context = TestContext::new();
        let _viewer = test_context.setup_kittest_for_rendering_3d([200.0, 100.0]);
        let render_state_guard = test_context.egui_render_state.lock();
        let render_state = render_state_guard.as_ref().unwrap();
        assert_eq!(
            render_state.adapter.get_info().device_type,
            wgpu::DeviceType::Cpu
        );
        assert_eq!(
            render_state.adapter.get_info().backend,
            wgpu::Backend::Vulkan
        );
    }
}
