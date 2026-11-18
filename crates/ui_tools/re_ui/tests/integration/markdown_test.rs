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

#[test]
pub fn test_markdown_inline_code() {
    let markdown = "\
Dataset `libero` (robot manipulation demos, from HDF5).

* Timelines: `sim_time`, `row_index`
* Cameras: `/camera/agentview` and `/camera/eye_in_hand` (wrist cam)
* Robot: `/urdf/transforms`, plus the `urdf-model` asset (shown in every segment)
* Demo data: `/demo`, and raw `__hdf5_properties` metadata

Some code:

```python
print(\"hello\")
```
";

    let mut results = egui_kittest::SnapshotResults::new();
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut harness =
            re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(500.0, 200.0))
                .with_theme(theme)
                .build_ui(|ui| ui.markdown_ui(markdown));
        harness.run();
        harness.fit_contents();
        harness.snapshot(format!("markdown_inline_code_{theme:?}").to_lowercase());
        results.extend_harness(&mut harness);
    }
}
