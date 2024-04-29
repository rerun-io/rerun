use crate::{drag_and_drop, hierarchical_drag_and_drop, selection_buttons};
use re_ui::{list_item2, ReUi};

pub struct RightPanel {
    show_hierarchical_demo: bool,
    drag_and_drop: drag_and_drop::ExampleDragAndDrop,
    hierarchical_drag_and_drop: hierarchical_drag_and_drop::HierarchicalDragAndDrop,
    selected_list_item: Option<usize>,
}

impl Default for RightPanel {
    fn default() -> Self {
        Self {
            show_hierarchical_demo: true,
            drag_and_drop: drag_and_drop::ExampleDragAndDrop::default(),
            hierarchical_drag_and_drop:
                hierarchical_drag_and_drop::HierarchicalDragAndDrop::default(),
            selected_list_item: None,
        }
    }
}

impl RightPanel {
    /// Draw the right panel content.
    ///
    /// Note: the panel's frame must have a zero inner margin!
    pub fn ui(&mut self, re_ui: &ReUi, ui: &mut egui::Ui) {
        //TODO(ab): remove this once ListItem 1.0 is phased out.
        ui.set_clip_rect(ui.max_rect());
        let background_x_range = ui.max_rect().x_range();

        //
        // First section - Drag and drop demos
        //

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            re_ui.panel_content(ui, |re_ui, ui| {
                re_ui.panel_title_bar_with_buttons(ui, "Drag-and-drop demo", None, |ui| {
                    ui.add(re_ui::toggle_switch(&mut self.show_hierarchical_demo));
                    ui.label("Hierarchical:");
                });

                if self.show_hierarchical_demo {
                    self.hierarchical_drag_and_drop.ui(re_ui, ui);
                } else {
                    self.drag_and_drop.ui(re_ui, ui);
                }
            });

            ReUi::full_span_separator(ui);
            ui.add_space(20.0);
        });

        //
        // Second section - no scroll area, so a single outer "panel_content" can be used.
        //

        re_ui.panel_content(ui, |re_ui, ui| {
            re_ui.large_collapsing_header(ui, "Full-Span UI examples", true, |ui| {
                ui.label("Some data here");
                ui.label("Some data there");

                selection_buttons(ui);
            });
        });

        // From now on, it's only `list_items`, so we need to remove the default
        // spacing.
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            //
            // Nested scroll area demo. Multiple `panel_content` must be used.
            //

            re_ui.panel_content(ui, |re_ui, ui| {
                re_ui.panel_title_bar(ui, "Another section", None);
            });

            egui::ScrollArea::both()
                .id_source("example_right_panel")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    re_ui.panel_content(ui, |re_ui, ui| {

                        for i in 0..10 {
                            let response = if i == 6 {
                                list_item2::ListItem::new(re_ui)
                                    .selected(Some(i) == self.selected_list_item)
                                    .show_hierarchical_with_children(
                                        ui,
                                        "item_with_children",
                                        true,
                                        list_item2::CustomListItemContent::new(|_, ui, context| ui.ctx().debug_painter().debug_rect(context.rect, egui::Color32::LIGHT_RED, "proudly produced by a CustomListItemContent's closure")),
                                        |re_ui, ui| {
                                            for _ in 0..3 {
                                                list_item2::ListItem::new(re_ui)
                                                    .interactive(false)
                                                    .show_hierarchical(
                                                        ui,
                                                        list_item2::BasicListItemContent::new("look ma a BasicListItemContent").with_icon(&re_ui::icons::SPACE_VIEW_2D),
                                                    );
                                            }
                                        },
                                    )
                                    .item_response
                            } else {
                                list_item2::ListItem::new(re_ui)
                                    .selected(Some(i) == self.selected_list_item)
                                    .show_hierarchical(ui, list_item2::DebugContent::default())
                            };

                            if response.clicked() {
                                self.selected_list_item = Some(i);
                            }
                        }

                        
                        // for i in 0..10 {
                        //     let label = if i == 4 {
                        //         "That's one heck of a loooooooong label!".to_owned()
                        //     } else {
                        //         format!("Some item {i}")
                        //     };
                        //
                        //     let mut item = re_ui
                        //         .list_item(label)
                        //         .selected(Some(i) == self.selected_list_item)
                        //         .interactive(i != 3)
                        //         .with_buttons(|re_ui, ui| {
                        //             re_ui.small_icon_button(ui, &re_ui::icons::ADD)
                        //                 | re_ui.small_icon_button(ui, &re_ui::icons::REMOVE)
                        //         });
                        //
                        //     // demo custom icon
                        //     item = if i == 6 {
                        //         item.with_icon_fn(|_re_ui, ui, rect, visuals| {
                        //             ui.painter().circle(
                        //                 rect.center(),
                        //                 rect.width() / 2.0,
                        //                 visuals.fg_stroke.color,
                        //                 egui::Stroke::NONE,
                        //             );
                        //         })
                        //     } else {
                        //         item.with_icon(&re_ui::icons::SPACE_VIEW_TEXT)
                        //     };
                        //
                        //     if item.show_flat(ui).clicked() {
                        //         self.selected_list_item = Some(i);
                        //     }
                        // }
                    });
                });

            //
            // Demo of `ListItem` features.
            //

            re_ui.panel_content(ui, |re_ui, ui| {
                re_ui.panel_title_bar(ui, "Another section", None);

                re_ui
                    .list_item("Collapsing list item with icon")
                    .with_icon(&re_ui::icons::SPACE_VIEW_2D)
                    .show_hierarchical_with_content(
                        ui,
                        "collapsing example",
                        true,
                        |_re_ui, ui| {
                            re_ui.list_item("Sub-item").show_hierarchical(ui);
                            re_ui.list_item("Sub-item").show_hierarchical(ui);
                            re_ui
                                .list_item("Sub-item with icon")
                                .with_icon(&re_ui::icons::SPACE_VIEW_TEXT)
                                .show_hierarchical(ui);
                            re_ui.list_item("Sub-item").show_hierarchical_with_content(
                                ui,
                                "sub-collapsing",
                                true,
                                |_re_ui, ui| re_ui.list_item("Sub-sub-item").show_hierarchical(ui),
                            );
                        },
                    );
            });
        });
    }
}
