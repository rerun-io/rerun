use egui::accesskit::Role;
use egui::{Key, Modifiers, Vec2};
use egui_kittest::kittest::Queryable as _;
use re_ui::text_edit::{ReTextEdit, TextEditExt as _, TextEditVariant};

type AddField = fn(&mut egui::Ui, &mut String) -> egui::Response;

/// Selected text must stay readable on the selection highlight, and the focus outline must keep
/// the field's color.
#[test]
pub fn test_text_edit_selection() {
    let cases: [(&str, AddField); 3] = [
        ("outlined", |ui, text| {
            ui.add(ReTextEdit::singleline(text).variant(TextEditVariant::Outlined))
        }),
        ("filled", |ui, text| {
            ui.add(ReTextEdit::singleline(text).variant(TextEditVariant::Filled))
        }),
        ("invalid", |ui, text| {
            ui.add(egui::TextEdit::singleline(text).invalid(true))
        }),
    ];

    let mut snapshot_results = egui_kittest::SnapshotResults::new();

    for (theme, theme_name) in [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")] {
        for (case_name, add_field) in cases {
            let mut text = "Selected and unselected".to_owned();

            let mut harness = egui_kittest::Harness::builder()
                .with_context_setup(re_ui::apply_style_and_install_loaders)
                .with_theme(theme)
                .with_size(Vec2::new(240.0, 40.0))
                .build_ui(|ui| {
                    ui.horizontal(|ui| {
                        let label = ui.label("Name");
                        add_field(ui, &mut text).labelled_by(label.id);
                    });
                });

            harness.get_by_role(Role::TextInput).focus();
            harness.run();
            harness.key_press(Key::Home);
            for _ in "Selected".chars() {
                harness.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
            }
            harness.run();

            snapshot_results
                .add(harness.try_snapshot(format!("text_edit_selection_{case_name}_{theme_name}")));
        }
    }
}

/// Text selected in a label must stay readable on the selection highlight.
#[test]
pub fn test_label_selection() {
    let mut snapshot_results = egui_kittest::SnapshotResults::new();

    for (theme, theme_name) in [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")] {
        let mut harness = egui_kittest::Harness::builder()
            .with_context_setup(re_ui::apply_style_and_install_loaders)
            .with_theme(theme)
            .with_size(Vec2::new(240.0, 40.0))
            .build_ui(|ui| {
                ui.label("Selected and unselected");
            });

        let rect = harness.get_by_label("Selected and unselected").rect();
        let start = rect.left_center();
        let end = start + Vec2::new(0.35 * rect.width(), 0.0);
        harness.drag_at(start);
        harness.run();
        harness.hover_at(end);
        harness.run();
        harness.drop_at(end);
        harness.run();

        snapshot_results.add(harness.try_snapshot(format!("label_selection_{theme_name}")));
    }
}
