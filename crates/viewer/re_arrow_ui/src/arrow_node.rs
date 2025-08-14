use crate::datatype_ui::data_type_ui;
use crate::show_index::ShowIndex;
use egui::{Id, RichText, Stroke, StrokeKind, Tooltip, Ui, WidgetText};
use re_format::format_uint;
use re_ui::UiExt as _;
use re_ui::list_item::{LabelContent, PropertyContent, list_item_scope};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

enum NodeLabel {
    Index(usize),
    Name(String),
    Custom(WidgetText),
}

pub struct ArrowNode<'a> {
    label: NodeLabel,
    values: &'a dyn ShowIndex,
}

impl<'a> ArrowNode<'a> {
    pub fn custom(name: impl Into<WidgetText>, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Custom(name.into()),
            values,
        }
    }

    pub fn name(name: impl Into<String>, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Name(name.into()),
            values,
        }
    }

    /// The index to *display*
    pub fn index(idx: usize, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Index(idx),
            values,
        }
    }

    /// The index of the *value* to display.
    /// Can be different from [`ArrowNode::index`] e.g. in a sliced array.
    pub fn show(self, ui: &mut Ui, index: usize) {
        let label = match self.label {
            NodeLabel::Index(idx) => {
                let mut builder = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
                builder.code_index(&format_uint(idx));
                builder.into_widget_text()
            }
            NodeLabel::Name(name) => {
                let mut builder = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
                builder.code_name(&name);
                builder.into_widget_text()
            }
            NodeLabel::Custom(name) => name,
        };

        let mut value = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
        let result = self.values.write(index, &mut value);
        let value = match result {
            Ok(()) => value.into_widget_text(),
            Err(e) => RichText::new(format!("Error: {e}"))
                .color(ui.tokens().error_fg_color)
                .into(),
        };

        let nested = self.values.is_item_nested();
        let data_type = self.values.array().data_type();
        let data_type_ui = data_type_ui(data_type);

        let item = ui.list_item();
        // We *don't* use index for the ID, since it might change across timesteps,
        // while referring the same logical data.
        let id = ui.id().with(label.text());
        let content = PropertyContent::new(label)
            .value_fn(|ui, visuals| {
                ui.horizontal(|ui| {
                    egui::Sides::new().shrink_left().show(
                        ui,
                        |ui| {
                            if visuals.is_collapsible() && visuals.openness() != 0.0 {
                                if visuals.openness() == 1.0 {
                                    return;
                                }
                                ui.set_opacity(1.0 - visuals.openness());
                            }
                            ui.label(value);
                        },
                        |ui| {
                            let tooltip_open =
                                Tooltip::was_tooltip_open_last_frame(ui.ctx(), ui.next_auto_id());
                            if visuals.hovered || tooltip_open {
                                let response =
                                    ui.small(RichText::new(&data_type_ui.type_name).strong());
                                ui.painter().rect_stroke(
                                    response.rect.expand(2.0),
                                    4.0,
                                    Stroke::new(1.0, visuals.text_color()),
                                    StrokeKind::Middle,
                                );

                                if let Some(content) = data_type_ui.content {
                                    response.on_hover_ui(|ui| {
                                        list_item_scope(
                                            ui,
                                            Id::new("arrow data type hover"),
                                            |ui| {
                                                ui.list_item().show_hierarchical_with_children(
                                                    ui,
                                                    Id::new("arrow data type item hover"),
                                                    true,
                                                    LabelContent::new(data_type_ui.type_name),
                                                    content,
                                                );
                                            },
                                        );
                                    });
                                }
                            }
                        },
                    );
                });
            })
            .show_only_when_collapsed(false);

        if nested {
            item.show_hierarchical_with_children(ui, id, false, content, |ui| {
                list_item_scope(ui, id.with("child_scope"), |ui| {
                    self.values.show(index, ui);
                });
            });
        } else {
            item.show_hierarchical(ui, content);
        }
    }
}
