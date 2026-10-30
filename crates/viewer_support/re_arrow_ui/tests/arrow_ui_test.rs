use std::f32::consts::PI;

use egui_kittest::SnapshotResults;
use re_ui::{UiExt as _, UiLayout};

mod arrow_test_data;

#[test]
pub fn test_arrow_ui() {
    let mut harness = egui_kittest::Harness::builder()
        .with_context_setup(re_ui::apply_style_and_install_loaders)
        .build_ui(|ui| {
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
    use re_sdk_types::encodings::{Utf8, Vec3D};

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
        (
            "Long string",
            [Utf8::from("This is a long root-level string that should not be truncated even though it is over 100 characters long, because it is the root element being displayed.")].to_arrow(),
        ),
        (
            "Two long strings",
            [Utf8::from("This is a long nested string that should be truncated because it is inside a list and over 100 characters long. Truncate me please!"), Utf8::from("Short string.")].to_arrow(),
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
            re_arrow_ui::arrow_ui(ui, UiLayout::SelectionPanel, Default::default(), &arrow);
        });

        harness.run();

        harness.fit_contents();

        harness.run();
        results.add(harness.try_snapshot(array_name));
    }
}

// Regression test for on-hover labels being cut off by scroll bars.
#[test]
fn hover_type_badge_in_scroll_area() {
    use egui_kittest::kittest::Queryable as _;

    let array = arrow::array::StringArray::from(vec!["value"; 5]);
    let mut results = SnapshotResults::new();
    for (name, margin) in [("flush", 0), ("padded", 16)] {
        let mut harness =
            re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, egui::vec2(300.0, 100.0))
                .build_ui(|ui| {
                    egui::Panel::right("selection_panel") // Makes the test more realistic for how this is usually used.
                        .default_size(284.0)
                        .frame(egui::Frame::NONE)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .content_margin(egui::Margin::same(margin))
                                .show(ui, |ui| {
                                    re_arrow_ui::arrow_ui(
                                        ui,
                                        UiLayout::SelectionPanel,
                                        Default::default(),
                                        &array,
                                    );
                                });
                        });
                });
        harness.get_by_label("0").hover();
        harness.run();
        results.add(harness.try_snapshot(format!("hover_type_badge_scrollbar_{name}")));
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

#[test]
fn truncated_value_tooltip() {
    use egui_kittest::kittest::Queryable as _;
    use re_sdk_types::ComponentBatch as _;
    use re_sdk_types::encodings::Utf8;

    let tests = [
        (
            "tooltip_long_multiline_string",
            Utf8::from(
                "Do any two tasks consecutively.\n\n\
                Suggested tasks:\n\
                * Move lid on or off of container (ex: pot, cup, pill bottle)\n\
                * Fold, spread out, or clump object (ex: cloth, charging cord, clothes)",
            )
            .to_arrow()
            .expect("Failed to convert to arrow"),
        ),
        (
            "tooltip_long_single_line_string",
            Utf8::from(
                "This string has no newlines, but it is much longer than the tooltip is wide, \
                so the tooltip must wrap it onto several rows instead of growing wider or cutting \
                it off again with an ellipsis like the table cell does.",
            )
            .to_arrow()
            .expect("Failed to convert to arrow"),
        ),
        (
            "tooltip_nested_struct",
            arrow_test_data::create_metadata_struct(1),
        ),
    ];

    let mut results = SnapshotResults::new();

    for (name, arrow) in tests {
        let mut harness = re_ui::testing::new_harness(
            re_ui::testing::TestOptions::Gui,
            egui::Vec2::new(700.0, 400.0),
        )
        .build_ui(|ui| {
            ui.set_max_width(250.0);
            re_arrow_ui::arrow_ui(ui, UiLayout::List, Default::default(), &arrow);
        });

        harness.run();
        harness.get_by_role(egui::accesskit::Role::Label).hover();
        harness.try_run_realtime().ok();

        results.add(harness.try_snapshot(name));
    }
}
