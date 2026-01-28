#![cfg(feature = "testing")]

use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use re_sdk_types::components::Colormap;
use re_test_context::TestContext;
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewer_context::{MaybeMutRef, ViewerContext};

/// Navigates from welcome to settings screen and snapshots it.
#[tokio::test]
async fn settings_screen() {
    #![expect(unsafe_code)] // It's only a test

    // SAFETY: it's only a test
    unsafe {
        std::env::set_var("TZ", "Europe/Stockholm");
    }

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1024.0, 1080.0)), // Settings screen can be a bit tall
        ..Default::default()
    });
    harness.get_by_label("Menu").click();
    harness.run_ok();
    harness.get_by_label_contains("Settings…").click();
    // Wait for the FFmpeg-check loading spinner to disappear.
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
    harness.snapshot("settings_screen");
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

            test_context.run(&ui.ctx().clone(), |ctx: &ViewerContext<'_>| {
                ui.horizontal(|ui| {
                    ui.label("Colormap:");

                    let mut test_colormap = Colormap::Spectral;
                    let mut colormap_ref = MaybeMutRef::MutRef(&mut test_colormap);

                    re_viewer_context::gpu_bridge::colormap_edit_or_view_ui(
                        ctx,
                        ui,
                        &mut colormap_ref,
                    );
                });
            });
        });

    harness.run();
    harness.fit_contents();
    harness.snapshot("colormap_selector_closed");

    // give the combo box some room to open
    harness.set_size(egui::Vec2::new(200.0, 350.0));
    harness.get_by_role(Role::ComboBox).click(); // open combo box
    harness.run();

    harness.fit_contents();
    harness.run();
    harness.snapshot("colormap_selector_open");
}
