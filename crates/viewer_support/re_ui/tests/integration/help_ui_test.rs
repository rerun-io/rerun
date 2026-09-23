use egui::os::OperatingSystem;
use egui::{Modifiers, vec2};
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, SnapshotResults};
use re_ui::{Help, IconText, MouseButtonText, UiExt as _, icons};

#[test]
fn test_help() {
    let mut snapshot_results = SnapshotResults::new();

    // We show different shortcuts based on the OS
    for os in [OperatingSystem::Windows, OperatingSystem::Mac] {
        let mut harness = Harness::builder()
            .with_size(vec2(240.0, 420.0))
            .build_ui(|ui| {
                ui.set_os(os);
                re_ui::apply_style_and_install_loaders(ui.ctx());

                ui.help_button(|ui| {
                    let mut help = Help::new("Help example")
                        .docs_link("https://rerun.io/docs/reference/types/views/map_view")
                        .control("Pan", (icons::LEFT_MOUSE_CLICK, "+", "drag"))
                        .control(
                            "Zoom",
                            IconText::from_modifiers_and(os, Modifiers::COMMAND, icons::SCROLL),
                        )
                        .control("Reset view", ("double", icons::LEFT_MOUSE_CLICK));

                    for modifier in [
                        Modifiers::ALT,
                        Modifiers::SHIFT,
                        Modifiers::CTRL,
                        Modifiers::COMMAND,
                        Modifiers::MAC_CMD,
                        Modifiers::NONE,
                    ] {
                        help = help.control(
                            format!("{modifier:?}"),
                            IconText::from_modifiers(os, modifier),
                        );
                    }

                    for btn in [
                        egui::PointerButton::Primary,
                        egui::PointerButton::Secondary,
                        egui::PointerButton::Middle,
                        egui::PointerButton::Extra1,
                        egui::PointerButton::Extra2,
                    ] {
                        help = help.control(format!("{btn:?}"), MouseButtonText(btn));
                    }

                    help.ui(ui);
                });
            });

        harness.get_by_label("Help").hover();

        harness.try_run_realtime().ok();

        snapshot_results.add(harness.try_snapshot(format!("help_{os:?}")));
    }
}
