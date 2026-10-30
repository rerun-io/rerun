//! Cmd-T / Ctrl-T opens a new conversation.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent::{AgentEntry, AgentProfile, AgentProfileId, AgentSettings};
use re_agent_ui::AgentPanel;

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
        profile_id: AgentProfileId::new("claude"),
        ..Default::default()
    };
    let mut panel = AgentPanel::with_agents(settings, None, agents, Vec::new());
    // A prompt makes it a conversation of its own, titled after it, which the panel then shows.
    panel
        .session_mut()
        .transcript_mut()
        .push_user("Hello".into());
    panel.poll_events();
    panel
}

fn harness() -> egui_kittest::Harness<'static, AgentPanel> {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel(),
        );
    harness.run_steps(2);
    harness
}

/// Only a conversation has a way back to the list.
fn shows_a_conversation(harness: &egui_kittest::Harness<'_, AgentPanel>) -> bool {
    harness.query_by_label("Back to conversations").is_some()
}

/// Focus a widget inside the panel, which is what scopes the shortcut to it.
fn focus_panel(harness: &mut egui_kittest::Harness<'_, AgentPanel>) {
    harness.get_by_label("More options").focus();
    harness.run_steps(2);
}

#[test]
fn shortcut_opens_a_new_conversation() {
    let mut harness = harness();
    focus_panel(&mut harness);
    assert!(harness.query_by_label("Hello").is_some());

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
    harness.run_steps(3);

    assert!(shows_a_conversation(&harness));
    assert!(harness.query_by_label("Hello").is_none());
}

/// The panel is one widget among many in its host: with the focus and the pointer elsewhere,
/// the host owns the shortcut.
#[test]
fn shortcut_outside_the_panel_is_ignored() {
    let mut harness = harness();

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
    harness.run_steps(3);

    assert!(shows_a_conversation(&harness));
}
