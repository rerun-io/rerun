use egui::{Id, RichText, Stroke, StrokeKind, Tooltip, Ui, WidgetText};
use re_format::format_uint;
use re_ui::list_item::{LabelContent, PropertyContent, list_item_scope};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{UiExt as _, UiLayout};

use crate::datatype_ui::DataTypeUi;
use crate::show_index::ShowIndex;

enum NodeLabel {
    /// The index to *display*. May be different from the actual index of the value.
    ///
    /// E.g. in a [`arrow::array::ListArray`], this is the index in the child list. The index passed to
    /// [`ArrowNode::show`] is the index in the parent array.
    ///
    /// Also see [`crate::show_index::list_ui`] for a more thorough explanation.
    Index(usize),
    Name(String),
    Custom(WidgetText),
}

/// Display an item of an Arrow array in a list item with some label.
pub struct ArrowNode<'a> {
    label: NodeLabel,
    values: &'a dyn ShowIndex,
}

impl<'a> ArrowNode<'a> {
    /// Create a new [`ArrowNode`] with a custom label
    pub fn custom(name: impl Into<WidgetText>, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Custom(name.into()),
            values,
        }
    }

    /// Create a new [`ArrowNode`] from an Arrow field.
    ///
    /// This will set the name to the fields name.
    pub fn field(field: &'a arrow::datatypes::Field, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Name(field.name().to_owned()),
            values,
        }
    }

    /// The index to *display* (See [`NodeLabel::Index`]).
    pub fn index(idx: usize, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Index(idx),
            values,
        }
    }

    /// Index is the index of the *value* to display.
    ///
    /// Can be different from [`ArrowNode::index`] (the index to display) e.g. in a sliced array.
    /// See also [`NodeLabel::Index`].
    pub fn show(self, ui: &mut Ui, index: usize) {
        let label = match self.label {
            NodeLabel::Index(idx) => {
                let mut builder = SyntaxHighlightedBuilder::new();
                builder.append_index(&format_uint(idx));
                builder.into_widget_text(ui.style())
            }
            NodeLabel::Name(name) => {
                let mut builder = SyntaxHighlightedBuilder::new();
                builder.append_identifier(&name);
                builder.into_widget_text(ui.style())
            }
            NodeLabel::Custom(name) => name,
        };

        let nested = self.values.is_item_nested();
        let data_type = self.values.array().data_type();
        let data_type_ui = DataTypeUi::new(data_type);

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
                            let mut value = SyntaxHighlightedBuilder::new();
                            let result = self.values.write(index, &mut value);

                            match result {
                                Ok(()) => UiLayout::List.data_label(ui, value),
                                Err(err) => ui.error_label(format!("Error: {err}")),
                            };
                        },
                        |ui| {
                            let tooltip_open =
                                Tooltip::was_tooltip_open_last_frame(ui.ctx(), ui.next_auto_id());
                            // Keep showing the data type when the tooltip is open, so the
                            // user can interact with it.
                            if visuals.hovered || tooltip_open {
                                // TODO(lucas): We should show the nullability here too
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
                                                    Id::new("arrow data type hover item"),
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
