use std::f32::consts::PI;

use egui_kittest::SnapshotResults;
use re_ui::{UiExt as _, UiLayout};

mod arrow_test_data;

#[test]
pub fn test_arrow_ui() {
    let mut harness = egui_kittest::Harness::builder().build_ui(|ui| {
        re_ui::apply_style_and_install_loaders(ui.ctx());

        arrow_list_ui(ui);
    });

    harness.run();

    harness.fit_contents();

    harness.run();
    harness.snapshot("arrow_ui");
}

fn arrow_list_ui(ui: &mut egui::Ui) {
    // We use a handful of realistic data in this test.

    use re_sdk_types::ComponentBatch as _;
    use re_sdk_types::components::Blob;
    use re_sdk_types::datatypes::{Utf8, Vec3D};

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
        ("String with newline", Utf8::from("Hello\nworld").to_arrow()),
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
            re_arrow_ui::arrow_ui(ui, UiLayout::List, Default::default(), &arrow);
            ui.end_row();
        }
    });
}

#[test]
fn arrow_tree_ui() {
    let arrays = arrow_test_data::all_arrays();

    let mut results = SnapshotResults::new();

    for (array_name, arrow) in arrays {
        let mut harness = re_ui::testing::new_harness(
            re_ui::testing::TestOptions::Gui,
            egui::Vec2::new(300.0, 500.0),
        )
        .build_ui(|ui| {
            re_ui::apply_style_and_install_loaders(ui.ctx());

            re_arrow_ui::arrow_ui(ui, UiLayout::SelectionPanel, Default::default(), &arrow);
        });

        harness.run();

        harness.fit_contents();

        harness.run();
        results.add(harness.try_snapshot(array_name));
    }
}

#[test]
fn inline_formatting() {
    for (name, data) in arrow_test_data::all_arrays() {
        let highlighted =
            re_arrow_ui::arrow_syntax_highlighted(&data).expect("Failed to format data");

        insta::assert_snapshot!(name, highlighted.text());
    }
}
