use crate::{drag_and_drop, hierarchical_drag_and_drop};
use egui::{Atom, Button, DragValue, RichText, TextEdit, Ui};
use re_ui::list_item::{ListItemContentButtonsExt as _, PropertyContent};
use re_ui::re_form::{ConstructFormStrip as _, FormFields, SelectableStrip, SelectableToggle};
use re_ui::{UiExt as _, icons, list_item};

pub struct RightPanel {
    show_hierarchical_demo: bool,
    drag_and_drop: drag_and_drop::ExampleDragAndDrop,
    hierarchical_drag_and_drop: hierarchical_drag_and_drop::HierarchicalDragAndDrop,
    selected_list_item: Option<usize>,
    use_action_button: bool,

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
            use_action_button: false,
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
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        //
        // First section - Drag and drop demos
        //

        ui.panel_content(|ui| {
            list_item::list_item_scope(ui, "drag_and_drop", |ui| {
                let show_hierarchical = self.show_hierarchical_demo;
                ui.section_collapsing_header("Drag and drop")
                    .with_buttons(|ui| {
                        ui.toggle_switch(8.0, &mut self.show_hierarchical_demo);
                        ui.label("Hierarchical:");
                    })
                    .show(ui, |ui| {
                        if show_hierarchical {
                            self.hierarchical_drag_and_drop.ui(ui);
                        } else {
                            self.drag_and_drop.ui(ui);
                        }
                    });
            });
        });

        //
        // Demo of `ListItem` API and features.
        //

        ui.panel_content(|ui| {
            list_item::list_item_scope(ui, "list_item_api", |ui| {
                Self::new_form_fields_demo(ui);

                ui.section_collapsing_header("List item api")
                    .show(ui, |ui| {
                        self.list_item_api_demo(ui);
                    });
            });
        });

        //
        // Nested scroll area demo. Multiple `panel_content` must be used to ensure the scroll
        // bar appears nicely snug with the panel right border.
        //

        ui.panel_content(|ui| {
            ui.panel_title_bar("Demo: ListItem in scroll area", None);
        });

        egui::ScrollArea::both()
            .id_salt("example_right_panel")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.panel_content(|ui| {
                    list_item::list_item_scope(ui, "scroll_area_demo", |ui| {
                        for i in 0..10 {
                            let label = if i == 4 {
                                "That's one heck of a loooooooong label!".to_owned()
                            } else {
                                format!("Some item {i}")
                            };

                            // Note: we use `truncate(false)` here to force the item to allocate
                            // as much as needed for the label, which in turn will trigger the
                            // scroll area.
                            if ui
                                .list_item()
                                .selected(Some(i) == self.selected_list_item)
                                .show_flat(ui, list_item::LabelContent::new(&label).truncate(false))
                                .clicked()
                            {
                                self.selected_list_item = Some(i);
                            }
                        }
                    });
                });
            });
    }

    fn list_item_api_demo(&mut self, ui: &mut Ui) {
        ui.list_item()
            .show_hierarchical(ui, list_item::LabelContent::new("Default"));

        ui.list_item()
            .interactive(false)
            .show_hierarchical(ui, list_item::LabelContent::new("Not interactive item"));

        ui.list_item()
            .force_hovered(true)
            .show_hierarchical(ui, list_item::LabelContent::new("Perma-hovered item"));

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
                        .with_icon(&re_ui::icons::VIEW_TEXT),
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
                            ui.re_radio_value(&mut self.boolean, true, "");
                        },
                    ),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("Fake radio button").with_icon_fn(
                        |ui, rect, _visuals| {
                            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                            ui.re_radio_value(&mut self.boolean, false, "");
                        },
                    ),
                );

                ui.list_item()
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new("LabelContent with custom styling")
                            .subdued(true)
                            .italics(true)
                            .with_icon(&re_ui::icons::VIEW_2D),
                    )
                    .on_hover_text("The styling applies to the icon.");

                ui.list_item()
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new("LabelContent with LabelStyle")
                            .label_style(re_ui::LabelStyle::Unnamed)
                            .with_icon(&re_ui::icons::VIEW_2D),
                    )
                    .on_hover_text("The LabelStyle doesn't apply to the icon.");

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with buttons").with_buttons(|ui| {
                        ui.small_icon_button(&re_ui::icons::ADD, "Add");
                        ui.small_icon_button(&re_ui::icons::REMOVE, "Remove");
                    }),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::LabelContent::new("LabelContent with buttons (always shown)")
                        .with_buttons(|ui| {
                            ui.small_icon_button(&re_ui::icons::ADD, "Add");
                            ui.small_icon_button(&re_ui::icons::REMOVE, "Remove");
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
                        list_item::PropertyContent::new("Bool").value_bool(self.boolean),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Bool (editable)")
                            .value_bool_mut(&mut self.boolean),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Text").value_text(&self.text),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Text (editable)")
                            .value_text_mut(&mut self.text),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Color")
                            .with_icon(&re_ui::icons::VIEW_TEXT)
                            .with_action_button(&re_ui::icons::ADD, "Add", || {
                                re_log::warn!("Add button clicked");
                            })
                            .value_color(&self.color)
                            .with_always_show_buttons(true),
                    );

                    ui.list_item().show_hierarchical(
                        ui,
                        list_item::PropertyContent::new("Color (editable)")
                            .with_icon(&re_ui::icons::VIEW_TEXT)
                            .with_action_button(&re_ui::icons::ADD, "Add", || {
                                re_log::warn!("Add button clicked");
                            })
                            .value_color_mut(&mut self.color)
                            .with_always_show_buttons(true),
                    );
                });
            },
        );

        ui.list_item().show_hierarchical_with_children(
            ui,
            ui.make_persistent_id("property content right button reserve"),
            true,
            list_item::PropertyContent::new("PropertyContent action button:")
                .value_text("demo of right gutter"),
            |ui| {
                // By using an inner scope, we allow the nested properties to not align themselves
                // to the parent property, which in this particular case looks better.
                list_item::list_item_scope(ui, "inner_scope", |ui| {
                    fn demo_item(ui: &mut egui::Ui) {
                        ui.list_item().show_hierarchical(
                            ui,
                            list_item::PropertyContent::new("Some item:").value_fn(|ui, _| {
                                ui.debug_painter().debug_rect(
                                    ui.max_rect(),
                                    egui::Color32::LIGHT_BLUE,
                                    "space for value",
                                );
                            }),
                        );
                    }

                    for _ in 0..3 {
                        demo_item(ui);
                    }

                    let mut content = list_item::PropertyContent::new("Use action button");
                    if self.use_action_button {
                        content = content.with_action_button(&re_ui::icons::ADD, "Add", || {
                            re_log::warn!("Add button clicked");
                        });
                    }
                    content = content.value_bool_mut(&mut self.use_action_button);
                    ui.list_item().show_hierarchical(ui, content);

                    for _ in 0..3 {
                        demo_item(ui);
                    }
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
                        ui.debug_painter().debug_rect(
                            ui.max_rect(),
                            egui::Color32::LIGHT_RED,
                            "CustomContent delegates to a closure",
                        );
                    }),
                );

                ui.list_item().show_hierarchical(
                    ui,
                    list_item::CustomContent::new(|ui, _| {
                        ui.debug_painter().debug_rect(
                            ui.max_rect(),
                            egui::Color32::LIGHT_RED,
                            "CustomContent with an action button",
                        );
                    })
                    .action_button(&re_ui::icons::ADD, "Add", || {
                        re_log::warn!("Add button clicked");
                    }),
                );
            },
        );
    }

    fn new_form_fields_demo(ui: &mut Ui) {
        list_item::list_item_scope(ui, "form_fields", |ui| {
            ui.section_collapsing_header("New form fields").show(ui, |ui| {
                ui.small("SeriesLines");

                ui.spacing_mut().item_spacing.y = 2.0; // TODO(lucas): 4 should be the correct value, why does 2 look right?
                let demo_ui = |ui: &mut Ui, content: PropertyContent<'_>| {
                    ui.list_item().interactive(false).show_hierarchical_with_children(
                        ui,
                        ui.next_auto_id(),
                        false,
                        content.with_action_button(&icons::MORE, "more", || {}).with_always_show_buttons(true),
                        |_ui| {},
                    );
                };

                let subdued = ui.tokens().text_subdued;
                let suffix = |suffix: &str| {
                    RichText::new(suffix).color(subdued).size(10.0)
                };

                demo_ui(
                    ui,
                    PropertyContent::new("width").value_fn(|ui, _vis| {
                        // TODO(RR-3883): Add atom support to dragvalue
                        FormFields::single(
                            ui,
                            Button::new((
                                "0.75",
                                suffix("pt"),
                            )),
                        );
                    }),
                );

                // TODO(RR-3864): ComboBox should allow customizing the icon globally
                let fake_combobox = |label: &str| {
                    Button::new((label, Atom::grow(), icons::COMBO_ARROW))
                        .image_tint_follows_text_color(true)
                };

                demo_ui(
                    ui,
                    PropertyContent::new("marker_size").value_fn(|ui, _vis| {
                        FormFields::relative(ui, [1.0, 2.0])
                            .and(DragValue::new(&mut 11.17))
                            .and(|ui: &mut Ui| {
                                // TODO(RR-3864): ComboBox shouldn't wrap button in a horizontal
                                // ComboBox::from_id_salt("123")
                                //     .selected_text("ui points")
                                //     .show_ui(ui, |ui| {})
                                //     .response
                                ui.add(
                                    fake_combobox("ui points")
                                )
                            });
                    }),
                );


                demo_ui(
                    ui,
                    PropertyContent::new("centers")
                        .value_fn(|ui, _vis| {
                            FormFields::same(ui, 2)
                                .and(Button::new((suffix("x"), "640")))
                                .and(Button::new((suffix("y"), "640")));
                        }),
                );

                demo_ui(
                    ui,
                    PropertyContent::new("range")
                        .value_fn(|ui, _vis| {
                            FormFields::same(ui, 2)
                                .and(Button::new(("640", Atom::grow(), suffix("min"))))
                                .and(Button::new(("640", Atom::grow(), suffix("max"))));
                        }),
                );

                demo_ui(
                    ui,
                    PropertyContent::new("centers")
                        .value_fn(|ui, _vis| {
                            FormFields::same(ui, 3)
                                .and(DragValue::new(&mut 2.0))
                                .and(DragValue::new(&mut 1.0))
                                .and(DragValue::new(&mut 0.0));
                        }),
                );

                demo_ui(
                    ui,
                    PropertyContent::new("vertex_normals")
                        .value_fn(|ui, _vis| {
                            FormFields::same(ui, 3)
                                .and(Button::new((suffix("x"), "640")))
                                .and(Button::new((suffix("y"), "640")))
                                .and(Button::new((suffix("z"), "640")));
                        }),
                );

                demo_ui(
                    ui,
                    PropertyContent::new("aggregation")
                        .value_fn(|ui, _vis| {
                            FormFields::single(ui, fake_combobox("Average"));
                        }),
                );

                demo_ui(
                    ui,
                    PropertyContent::new("visibility")
                        .value_fn(|ui, _vis| {
                            SelectableStrip::same(ui, 2)
                                .and(SelectableToggle::new("Show", true))
                                .and(SelectableToggle::new("Hide", false));
                        }),
                );

                demo_ui(
                    ui,
                    PropertyContent::new("name")
                        .value_fn(|ui, _vis| {
                            // TODO(RR-3859): TextEdit margin is hardcoded in egui. TextEdit also ignores
                            // interact size (should use it as min height)
                            FormFields::single(ui, TextEdit::singleline(&mut "Temperature".to_owned()));
                        }),
                );

                demo_ui(
                    ui,
                    PropertyContent::new("text")
                        .value_fn(|ui, _vis| {
                            // TODO(RR-3858): Listitems don't grow when their contents exceed their size
                            let mut text = "Give a bit of space to a longer text input fields, let’s say at least 3 lines and the rest …".to_owned();
                            FormFields::single(ui, TextEdit::multiline(&mut text).desired_rows(1));
                        }),
                );
            });
        });
    }
}
