use egui::Vec2;
use re_ui::UiExt as _;

/// Anything drawn past the right edge of a [`egui::Ui`] widens it, and inside a resizable panel
/// that feeds back into the next frame, growing the panel until it fills the window.
#[test]
pub fn test_markdown_code_block_stays_within_its_ui() {
    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(300.0, 200.0))
            .build_ui_state(
                |ui, overhang: &mut f32| {
                    let max_right = ui.max_rect().right();
                    ui.markdown_ui("Some code:\n\n```python\nprint(\"hello\")\n```\n");
                    *overhang = overhang.max(ui.min_rect().right() - max_right);
                },
                0.0,
            );

    harness.run();

    let overhang = *harness.state();
    assert!(
        overhang <= 0.0,
        "The code block reaches {overhang} px past the right edge of its ui"
    );
}
