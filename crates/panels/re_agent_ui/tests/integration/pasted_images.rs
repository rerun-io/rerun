//! Images pasted into the composer.

use egui::{Role, Vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_agent_ui::AgentPanel;

use crate::chat_input::ready_panel;

/// A panel whose agent is mid-turn, so a prompt is queued rather than sent to a process that
/// does not exist in a test.
///
/// Mid-turn, the panel shows a spinner that requests a repaint every frame, so `Harness::run`
/// never settles and panics once it exceeds its step limit. Tests on this panel step a fixed
/// number of frames instead.
fn queued_panel() -> AgentPanel {
    let mut panel = ready_panel();
    panel
        .session_mut()
        .expect("one conversation")
        .begin_test_turn();
    panel
}

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0);

/// A small opaque image, standing in for whatever the clipboard holds.
fn clipboard_image() -> std::sync::Arc<egui::ColorImage> {
    #[expect(clippy::disallowed_methods)] // a test fixture, not part of the theme
    let color = egui::Color32::from_rgb(10, 20, 30);
    std::sync::Arc::new(egui::ColorImage::filled([4, 3], color))
}

/// Pasting attaches the image to the prompt being composed, and sending clears it again.
#[test]
fn a_pasted_image_is_attached_to_the_next_prompt() {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            queued_panel(),
        );
    // A fixed number of steps rather than `run()`: the spinner keeps the harness from ever
    // settling. See `queued_panel`.
    harness.run_steps(2);

    let input = harness.get_by_role(Role::MultilineTextInput);
    input.focus();
    input.type_text("what is this?");
    harness.run_steps(2);

    harness
        .input_mut()
        .events
        .push(egui::Event::PasteImage(clipboard_image()));
    // The composer grows upward when the preview appears, so let the layout settle before
    // asking where anything is.
    harness.run_steps(4);

    // The preview is only there while the image is attached, so its remove button stands in
    // for the attachment itself.
    harness.get_by_label("Remove this image");

    harness.key_press(egui::Key::Enter);
    harness.run_steps(4);

    assert!(
        harness.query_by_label("Remove this image").is_none(),
        "the image outlived the prompt it was attached to"
    );
    let session = harness.state().session().expect("one conversation");
    let queued = session.queued_prompts().front().expect("one queued prompt");
    assert_eq!(queued.text, "what is this?");
    assert_eq!(queued.images.len(), 1);
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert_eq!(input.accesskit_node().value(), Some(String::new()));
}

/// The remove button drops an image without touching the text.
#[test]
fn a_pasted_image_can_be_removed_again() {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            ready_panel(),
        );
    harness.run();

    let input = harness.get_by_role(Role::MultilineTextInput);
    input.focus();
    input.type_text("keep this text");
    harness.run();

    harness
        .input_mut()
        .events
        .push(egui::Event::PasteImage(clipboard_image()));
    harness.run();

    harness.get_by_label("Remove this image").click();
    harness.run();

    assert!(harness.query_by_label("Remove this image").is_none());
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert_eq!(
        input.accesskit_node().value(),
        Some("keep this text".into())
    );
}
