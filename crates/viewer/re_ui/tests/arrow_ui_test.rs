#![cfg(feature = "arrow")]

use std::f32::consts::PI;

use re_ui::{UiExt as _, UiLayout};

#[test]
pub fn test_arrow_ui() {
    let mut harness = egui_kittest::Harness::builder().build_ui(|ui| {
        re_ui::apply_style_and_install_loaders(ui.ctx());

        show_some_arrow_ui(ui);
    });

    harness.run();

    harness.fit_contents();

    harness.run();
    harness.snapshot("arrow_ui");
}

fn show_some_arrow_ui(ui: &mut egui::Ui) {
    // We use a handful of realistic data in this test.

    use re_types::{
        LoggableBatch as _,
        components::Blob,
        datatypes::{Utf8, Vec3D},
    };

    let tests = [
        ("Empty string", Utf8::from("").to_arrow()),
        ("One string", Utf8::from("Hello world").to_arrow()),
        (
            "Multiline string",
            Utf8::from(
                "First line.\n\
                The second line has a \ttab.\n\
                The final third line",
            )
            .to_arrow(),
        ),
        (
            "Special characters",
            Utf8::from(r#"With \backslash, "quotes", thin space"#).to_arrow(),
        ),
        (
            "Two strings",
            [Utf8::from("Hello"), Utf8::from("world")].to_arrow(),
        ),
        ("String with URL", Utf8::from("https://rerun.io").to_arrow()),
        (
            "Two URLs in strings",
            [
                Utf8::from("https://rerun.io"),
                Utf8::from("https://rerun.rs"),
            ]
            .to_arrow(),
        ),
        ("Empty Blob", Blob::from(vec![]).to_arrow()),
        ("Small Blob", Blob::from(vec![1, 2, 3, 4]).to_arrow()),
        ("Big Blob", Blob::from(vec![42; 1_234_567]).to_arrow()),
        ("Vec3", Vec3D::from([13.37, 42.0, PI]).to_arrow()),
    ];

    egui::Grid::new("entity_db").num_columns(2).show(ui, |ui| {
        ui.strong("What");
        ui.strong("arrow_ui");
        ui.end_row();

        for (name, arrow_result) in tests {
            ui.grid_left_hand_label(name);
            let arrow = arrow_result.expect("Failed to convert to arrow");
            re_ui::arrow_ui(ui, UiLayout::List, &arrow);
            ui.end_row();
        }
    });
}
