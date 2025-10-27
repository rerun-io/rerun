#![cfg(feature = "testing")]

use egui_kittest::kittest::Queryable as _;
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{UiExt as _, icons, list_item};

#[test]
pub fn test_list_items_should_match_snapshot() {
    let mut boolean = true;
    let mut text = String::from("hello");
    let mut color = [255, 255, 0, 255];

    let mut test_code = |ui: &mut egui::Ui| {
        ui.list_item()
            .header()
            .show_hierarchical(ui, list_item::LabelContent::header("Header item"));

        ui.list_item()
            .show_hierarchical(ui, list_item::LabelContent::new("Default"));

        ui.list_item()
            .interactive(false)
            .show_hierarchical(ui, list_item::LabelContent::new("Not interactive item"));

        ui.list_item()
            .force_hovered(true)
            .show_hierarchical(ui, list_item::LabelContent::new("Perma-hovered item"));

        ui.list_item()
            .show_hierarchical(ui, list_item::LabelContent::new("Focused item"));

        ui.list_item().show_hierarchical_with_children(
            ui,
            ui.make_persistent_id("label content features"),
            true,
            list_item::LabelContent::new("LabelContent features:"),
            |ui| {
                ui.list_item()
                    .show_hierarchical(ui, list_item::LabelContent::new("LabelContent"));

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with icon")
                        .with_icon(&icons::VIEW_TEXT),
                );

                ui.list_item().active(true).show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with icon + active")
                        .with_icon(&icons::VIEW_TEXT),
                );

                ui.list_item().selected(true).show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with icon + selected")
                        .with_icon(&icons::VIEW_TEXT),
                );

                ui.list_item().force_hovered(true).show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with icon + hovered")
                        .with_icon(&icons::VIEW_TEXT),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with custom icon code")
                        .with_icon_fn(|ui, rect, visuals| {
                            ui.painter().circle(
                                rect.center(),
                                rect.width() / 2.0,
                                visuals.icon_tint(),
                                egui::Stroke::NONE,
                            );
                        }),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("Fake radio button").with_icon_fn(
                        |ui, rect, _visuals| {
                            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                            ui.re_radio_value(&mut boolean, true, "");
                        },
                    ),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("Fake radio button").with_icon_fn(
                        |ui, rect, _visuals| {
                            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                            ui.re_radio_value(&mut boolean, false, "");
                        },
                    ),
                );

                ui.list_item()
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new("LabelContent with custom styling")
                            .subdued(true)
                            .italics(true)
                            .with_icon(&icons::VIEW_2D),
                    )
                    .on_hover_text("The styling applies to the icon.");

                ui.list_item()
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new("LabelContent with LabelStyle")
                            .label_style(re_ui::LabelStyle::Unnamed)
                            .with_icon(&icons::VIEW_2D),
                    )
                    .on_hover_text("The LabelStyle doesn't apply to the icon.");

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with buttons").with_buttons(|ui| {
                        ui.small_icon_button(&icons::ADD, "Add");
                        ui.small_icon_button(&icons::REMOVE, "Remove");
                    }),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with buttons (always shown)")
                        .with_buttons(|ui| {
                            ui.small_icon_button(&icons::ADD, "Add");
                            ui.small_icon_button(&icons::REMOVE, "Remove");
                        })
                        .with_always_show_buttons(true),
                );
            },
        );

        ui.list_item().show_hierarchical_with_children(
            ui,
            ui.make_persistent_id("property content features"),
            true,
            list_item::PropertyContent::new("PropertyContent features:")
                .value_text("bunch of properties"),
            |ui| {
                // By using an inner scope, we allow the nested properties to not align themselves
                // to the parent property, which in this particular case looks better.
                list_item::list_item_scope(ui, "inner_scope", |ui| {
                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Bool").value_bool(boolean),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Bool (editable)")
                            .value_bool_mut(&mut boolean),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Text").value_text(&text),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Text (editable)")
                            .value_text_mut(&mut text),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Color")
                            .with_icon(&icons::VIEW_TEXT)
                            .with_action_button(&icons::ADD, "Add", || {})
                            .value_color(&color)
                            .with_always_show_buttons(true),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Color (editable)")
                            .with_icon(&icons::VIEW_TEXT)
                            .with_action_button(&icons::ADD, "Add", || {})
                            .value_color_mut(&mut color)
                            .with_always_show_buttons(true),
                    );
                });
            },
        );

        ui.list_item().show_hierarchical_with_children(
            ui,
            ui.make_persistent_id("other features"),
            true,
            list_item::LabelContent::new("Other contents:"),
            |ui| {
                ui.list_item().show_hierarchical(
                    ui,
                    list_item::DebugContent::default()
                        .label("DebugContent just shows the content area"),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::CustomContent::new(|ui, _| {
                        ui.ctx().debug_painter().debug_rect(
                            ui.max_rect(),
                            egui::Color32::LIGHT_RED,
                            "CustomContent delegates to a closure",
                        );
                    }),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::CustomContent::new(|ui, _| {
                        ui.ctx().debug_painter().debug_rect(
                            ui.max_rect(),
                            egui::Color32::LIGHT_RED,
                            "CustomContent with an action button",
                        );
                    })
                    .action_button(&icons::ADD, "Add", || {}),
                );
            },
        );
    };

    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, [700.0, 700.0])
        .build(|ctx| {
            egui::SidePanel::right("right_panel").show(ctx, |ui| {
                ui.set_width(650.0);
                ui.set_max_width(650.0);
                re_ui::apply_style_and_install_loaders(ui.ctx());
                list_item::list_item_scope(ui, "list_item_scope", &mut test_code);
            });
        });

    harness.get_by_label("Focused item").focus();

    harness.run();
    harness.snapshot("list_items");
}
