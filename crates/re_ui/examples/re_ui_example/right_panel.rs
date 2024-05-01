use crate::{drag_and_drop, hierarchical_drag_and_drop};
use egui::Ui;
use re_ui::{list_item2, ReUi};

pub struct RightPanel {
    show_hierarchical_demo: bool,
    drag_and_drop: drag_and_drop::ExampleDragAndDrop,
    hierarchical_drag_and_drop: hierarchical_drag_and_drop::HierarchicalDragAndDrop,
    selected_list_item: Option<usize>,

    // dummy data
    text: String,
    color: [u8; 4],
    boolean: bool,
}

impl Default for RightPanel {
    fn default() -> Self {
        Self {
            show_hierarchical_demo: true,
            drag_and_drop: drag_and_drop::ExampleDragAndDrop::default(),
            hierarchical_drag_and_drop:
                hierarchical_drag_and_drop::HierarchicalDragAndDrop::default(),
            selected_list_item: None,
            // dummy data
            text: "Hello world".to_owned(),
            color: [128, 0, 0, 255],
            boolean: false,
        }
    }
}

impl RightPanel {
    /// Draw the right panel content.
    ///
    /// Note: the panel's frame must have a zero inner margin and the vertical spacing set to 0.
    pub fn ui(&mut self, re_ui: &ReUi, ui: &mut egui::Ui) {
        //
        // First section - Drag and drop demos
        //

        re_ui.panel_content(ui, |re_ui, ui| {
            re_ui.panel_title_bar_with_buttons(ui, "Demo: drag-and-drop", None, |ui| {
                ui.add(re_ui::toggle_switch(8.0, &mut self.show_hierarchical_demo));
                ui.label("Hierarchical:");
            });

            if self.show_hierarchical_demo {
                self.hierarchical_drag_and_drop.ui(re_ui, ui);
            } else {
                self.drag_and_drop.ui(re_ui, ui);
            }
        });

        ui.add_space(20.0);

        //
        // Demo of `ListItem` API and features.
        //

        re_ui.panel_content(ui, |re_ui, ui| {
            re_ui.panel_title_bar(ui, "Demo: ListItem APIs", None);
            self.list_item_api_demo(re_ui, ui);
        });

        ui.add_space(20.0);

        //
        // Nested scroll area demo. Multiple `panel_content` must be used to ensure the scroll
        // bar appears nicely snug with the panel right border.
        //

        re_ui.panel_content(ui, |re_ui, ui| {
            re_ui.panel_title_bar(ui, "Demo: ListItem in scroll area", None);
        });

        egui::ScrollArea::both()
            .id_source("example_right_panel")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                re_ui.panel_content(ui, |re_ui, ui| {
                    for i in 0..10 {
                        let label = if i == 4 {
                            "That's one heck of a loooooooong label!".to_owned()
                        } else {
                            format!("Some item {i}")
                        };

                        // Note: we use `exact_width(true)` here to force the item to allocate
                        // as much as needed for the label, which in turn will trigger the
                        // scroll area.
                        if re_ui
                            .list_item2()
                            .selected(Some(i) == self.selected_list_item)
                            .show_flat(ui, list_item2::LabelContent::new(&label).exact_width(true))
                            .clicked()
                        {
                            self.selected_list_item = Some(i);
                        }
                    }
                });
            });
    }

    fn list_item_api_demo(&mut self, re_ui: &ReUi, ui: &mut Ui) {
        re_ui
            .list_item2()
            .show_hierarchical(ui, list_item2::LabelContent::new("Default"));

        re_ui
            .list_item2()
            .interactive(false)
            .show_hierarchical(ui, list_item2::LabelContent::new("Not interactive item"));

        re_ui
            .list_item2()
            .force_hovered(true)
            .show_hierarchical(ui, list_item2::LabelContent::new("Perma-hovered item"));

        re_ui.list_item2().show_hierarchical_with_children(
            ui,
            "label content features",
            true,
            list_item2::LabelContent::new("LabelContent features:"),
            |re_ui, ui| {
                re_ui
                    .list_item2()
                    .show_hierarchical(ui, list_item2::LabelContent::new("LabelContent"));

                re_ui.list_item2().show_hierarchical(
                    ui,
                    list_item2::LabelContent::new("LabelContent with icon")
                        .with_icon(&re_ui::icons::SPACE_VIEW_TEXT),
                );

                re_ui.list_item2().show_hierarchical(
                    ui,
                    list_item2::LabelContent::new("LabelContent with custom icon code")
                        .with_icon_fn(|_re_ui, ui, rect, visuals| {
                            ui.painter().circle(
                                rect.center(),
                                rect.width() / 2.0,
                                visuals.fg_stroke.color,
                                egui::Stroke::NONE,
                            );
                        }),
                );

                re_ui
                    .list_item2()
                    .show_hierarchical(
                        ui,
                        list_item2::LabelContent::new("LabelContent with custom styling")
                            .subdued(true)
                            .italics(true)
                            .with_icon(&re_ui::icons::SPACE_VIEW_2D),
                    )
                    .on_hover_text("The styling applies to the icon.");

                re_ui
                    .list_item2()
                    .show_hierarchical(
                        ui,
                        list_item2::LabelContent::new("LabelContent with LabelStyle")
                            .label_style(re_ui::LabelStyle::Unnamed)
                            .with_icon(&re_ui::icons::SPACE_VIEW_2D),
                    )
                    .on_hover_text("The LabelStyle doesn't apply to the icon.");

                re_ui.list_item2().show_hierarchical(
                    ui,
                    list_item2::LabelContent::new("LabelContent with buttons").with_buttons(
                        |re_ui, ui| {
                            re_ui.small_icon_button(ui, &re_ui::icons::ADD)
                                | re_ui.small_icon_button(ui, &re_ui::icons::REMOVE)
                        },
                    ),
                );
            },
        );

        re_ui.list_item2().show_hierarchical_with_children(
            ui,
            "property content features",
            true,
            list_item2::PropertyContent::new("PropertyContent features:")
                .value_text("bunch of properties"),
            |re_ui, ui| {
                // By using an inner scope, we allow the nested properties to not align themselves
                // to the parent property, which in this particular case looks better.
                list_item2::list_item_scope(ui, "inner_scope", None, |ui| {
                    re_ui.list_item2().show_hierarchical(
                        ui,
                        list_item2::PropertyContent::new("Bool").value_bool(self.boolean),
                    );

                    re_ui.list_item2().show_hierarchical(
                        ui,
                        list_item2::PropertyContent::new("Bool (editable)")
                            .value_bool_mut(&mut self.boolean),
                    );

                    re_ui.list_item2().show_hierarchical(
                        ui,
                        list_item2::PropertyContent::new("Text").value_text(&self.text),
                    );

                    re_ui.list_item2().show_hierarchical(
                        ui,
                        list_item2::PropertyContent::new("Text (editable)")
                            .value_text_mut(&mut self.text),
                    );

                    re_ui.list_item2().show_hierarchical(
                        ui,
                        list_item2::PropertyContent::new("Color")
                            .with_icon(&re_ui::icons::SPACE_VIEW_TEXT)
                            .action_button(&re_ui::icons::ADD, || {
                                re_log::warn!("Add button clicked");
                            })
                            .value_color(&self.color),
                    );

                    re_ui.list_item2().show_hierarchical(
                        ui,
                        list_item2::PropertyContent::new("Color (editable)")
                            .with_icon(&re_ui::icons::SPACE_VIEW_TEXT)
                            .action_button(&re_ui::icons::ADD, || {
                                re_log::warn!("Add button clicked");
                            })
                            .value_color_mut(&mut self.color),
                    );
                });
            },
        );

        re_ui.list_item2().show_hierarchical_with_children(
            ui,
            "other features",
            true,
            list_item2::LabelContent::new("Other contents:"),
            |re_ui, ui| {
                re_ui.list_item2().show_hierarchical(
                    ui,
                    list_item2::LabelContent::new("next line is a EmptyContent:")
                        .subdued(true)
                        .italics(true),
                );

                re_ui
                    .list_item2()
                    .show_hierarchical(ui, list_item2::EmptyContent);

                re_ui.list_item2().show_hierarchical(
                    ui,
                    list_item2::DebugContent::default()
                        .label("DebugContent just shows the content area"),
                );

                re_ui.list_item2().show_hierarchical(
                    ui,
                    list_item2::CustomContent::new(|_, ui, context| {
                        ui.ctx().debug_painter().debug_rect(
                            context.rect,
                            egui::Color32::LIGHT_RED,
                            "CustomContent delegates to a closure",
                        );
                    }),
                )
            },
        );
    }
}
