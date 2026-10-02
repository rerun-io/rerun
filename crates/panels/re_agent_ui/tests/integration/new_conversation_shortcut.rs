//! Cmd-T / Ctrl-T opens another conversation tab.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent_ui::{AgentEntry, AgentPanel, AgentProfile, AgentSettings};

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0);

fn panel() -> AgentPanel {
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            executable: Some(PathBuf::from("/usr/local/bin").join(&profile.command)),
            profile,
        })
        .collect();
    let settings = AgentSettings {
        profile_id: "claude".to_owned(),
        ..Default::default()
    };
    AgentPanel::with_agents(settings, agents)
}

fn harness() -> egui_kittest::Harness<'static, AgentPanel> {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            panel(),
        );
    harness.run_steps(2);
    harness
}

/// Every conversation tab has its own close button.
fn tab_count(harness: &egui_kittest::Harness<'_, AgentPanel>) -> usize {
    harness.query_all_by_label("Close").count()
}

/// Focus a widget inside the panel, which is what scopes the shortcut to it.
fn focus_panel(harness: &mut egui_kittest::Harness<'_, AgentPanel>) {
    harness.get_by_label("New conversation").focus();
    harness.run_steps(2);
}

#[test]
fn shortcut_opens_another_tab() {
    let mut harness = harness();
    focus_panel(&mut harness);
    assert_eq!(tab_count(&harness), 1);

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
    harness.run_steps(3);

    assert_eq!(tab_count(&harness), 2);
}

/// The panel is one widget among many in its host: with the focus and the pointer elsewhere,
/// the host owns the shortcut.
#[test]
fn shortcut_outside_the_panel_is_ignored() {
    let mut harness = harness();

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
    harness.run_steps(3);

    assert_eq!(tab_count(&harness), 1);
}
