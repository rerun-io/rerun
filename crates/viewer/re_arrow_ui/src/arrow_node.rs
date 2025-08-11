use crate::child_nodes;
use crate::child_nodes::ChildNodes;
use crate::datatype_ui::datatype_ui;
use arrow::datatypes::DataType;
use egui::text::LayoutJob;
use egui::{Id, Response, RichText, Stroke, StrokeKind, TextFormat, TextStyle, Ui, WidgetText};
use re_ui::UiExt;
use re_ui::list_item::{LabelContent, PropertyContent, list_item_scope};
use std::ops::Range;

pub struct ArrowNode<'a> {
    array: child_nodes::MaybeArc<'a>,
    index: usize,
    field_name: Option<WidgetText>, // TODO: Can be &str?
    child_nodes: Option<ChildNodes<'a>>,
}

impl<'a> ArrowNode<'a> {
    pub fn new(
        array: impl Into<child_nodes::MaybeArc<'a>>,
        index: usize,
        child_nodes: Option<ChildNodes<'a>>,
    ) -> Self {
        ArrowNode {
            array: array.into(),
            index,
            field_name: None,
            child_nodes,
        }
    }

    pub fn with_field_name(mut self, field_name: impl Into<WidgetText>) -> Self {
        self.field_name = Some(field_name.into());
        self
    }

    fn layout_job(&self, ui: &Ui) -> LayoutJob {
        let mut job = LayoutJob::default();
        self.inline_value_layout_job(&mut job, ui);
        job
    }

    fn text_format_base(ui: &Ui) -> TextFormat {
        TextFormat {
            font_id: TextStyle::Monospace.resolve(ui.style()),
            color: ui.tokens().text_default,
            ..Default::default()
        }
    }

    fn text_format_strong(ui: &Ui) -> TextFormat {
        let mut format = Self::text_format_base(ui);
        format.color = ui.tokens().text_strong;
        format
    }
    fn text_format_number(ui: &Ui) -> TextFormat {
        let mut format = Self::text_format_base(ui);
        format.color = ui.tokens().code_number;
        format
    }
    fn text_format_string(ui: &Ui) -> TextFormat {
        let mut format = Self::text_format_base(ui);
        format.color = ui.tokens().code_string;
        format
    }

    fn inline_value_layout_job(&self, job: &mut LayoutJob, ui: &Ui) {
        let format_strong = Self::text_format_strong(ui);
        let format_base = Self::text_format_base(ui);

        if let Some(children) = self.child_nodes.clone() {
            let len = children.len();
            const MAX_INLINE_ITEMS: usize = 3;

            let mut peekable = children
                .iter()
                // Limit the number of items we show inline
                .take(MAX_INLINE_ITEMS)
                .peekable();
            // TODO: Add some fallback in case it's empty
            let has_name = peekable
                .peek()
                .is_some_and(|node| node.field_name.is_some());

            if !has_name {
                job.append(&format!("({len}) "), 0.0, format_base.clone());
            }

            let (open, close) = if has_name { ("{", "}") } else { ("[", "]") };
            job.append(open, 0.0, format_strong.clone());

            while let Some(child) = peekable.next() {
                if let Some(field_name) = &child.field_name {
                    job.append(field_name.text(), 0.0, format_base.clone());
                    job.append(": ", 0.0, format_strong.clone());
                }
                child.inline_value_layout_job(job, ui);
                if peekable.peek().is_some() {
                    job.append(", ", 0.0, format_strong.clone());
                }
            }

            if len > MAX_INLINE_ITEMS {
                job.append(", ", 0.0, format_strong.clone());
                job.append("…", 0.0, format_strong.clone());
            }

            job.append(close, 0.0, format_strong.clone());
        } else {
            let mut value =
                if let Ok(formatter) = crate::arrow_ui::make_formatter(self.array.as_ref()) {
                    formatter(self.index)
                } else {
                    "Error formatting value".to_owned()
                };

            let data_type: &DataType = self.array.as_ref().data_type();
            let format = if matches!(
                data_type,
                DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
            ) {
                value = format!("\"{value}\"");
                Self::text_format_string(ui)
            } else {
                Self::text_format_number(ui)
            };
            job.append(&value, 0.0, format);
        }
    }

    pub fn ui(self, ui: &mut Ui) -> Response {
        let ArrowNode {
            array,
            index,
            field_name,
            child_nodes,
        } = &self;

        let array = child_nodes::MaybeArc::as_ref(array);

        let data_type_name = datatype_ui(array.data_type()).0;

        let item = ui.list_item();

        let text = field_name
            .clone()
            .map(|f| f.color(ui.tokens().text_default))
            .unwrap_or_else(|| {
                RichText::new(index.to_string())
                    .color(ui.tokens().code_index)
                    .into()
            })
            .monospace();
        let id = ui.unique_id().with(text.text());

        let job = self.layout_job(ui);

        let content = PropertyContent::new(text)
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
                            ui.label(job);
                        },
                        |ui| {
                            if visuals.hovered {
                                let response = ui.small(RichText::new(data_type_name).strong());
                                ui.painter().rect_stroke(
                                    response.rect.expand(2.0),
                                    4.0,
                                    Stroke::new(1.0, visuals.text_color()),
                                    StrokeKind::Middle,
                                );

                                response.on_hover_ui(|ui| {
                                    let (datatype_name, maybe_ui) = datatype_ui(array.data_type());
                                    if let Some(content) = maybe_ui {
                                        list_item_scope(
                                            ui,
                                            Id::new("arrow data type hover"),
                                            |ui| {
                                                ui.list_item().show_hierarchical_with_children(
                                                    ui,
                                                    Id::new("arrow data type item hover"),
                                                    true,
                                                    LabelContent::new(datatype_name),
                                                    content,
                                                );
                                            },
                                        );
                                    }
                                });
                            }
                        },
                    );
                });
            })
            .show_only_when_collapsed(false);

        let response = if let Some(children) = child_nodes {
            item.show_hierarchical_with_children(ui, id, false, content, |ui| {
                // We create a new scope so properties are only aligned on a single level
                ui.list_item_scope(id.with("scope"), |ui| {
                    let len = children.len();
                    let range = 0..len;
                    list_item_ranges(ui, range, &mut |ui, i| {
                        children.get_child(i).ui(ui);
                    });
                });
            })
            .item_response
        } else {
            item.show_hierarchical(ui, content)
        };

        response
    }
}

fn list_item_ranges(ui: &mut Ui, range: Range<usize>, item_fn: &mut dyn FnMut(&mut Ui, usize)) {
    let range_len = range.len();

    const RANGE_SIZE: usize = 100;

    if range_len <= RANGE_SIZE {
        for i in range {
            item_fn(ui, i);
        }
        return;
    }

    let chunk_size = if range_len <= 10_000 {
        100
    } else if range_len <= 1_000_000 {
        10_000
    } else if range_len <= 100_000_000 {
        1_000_000
    } else {
        100_000_000
    };

    let mut current = range.start;
    while current < range.end {
        let chunk_end = usize::min(current + chunk_size, range.end);
        let chunk_range = current..chunk_end;
        let id = ui.unique_id().with(chunk_range.clone());
        ui.list_item().show_hierarchical_with_children(
            ui,
            id,
            false,
            LabelContent::new(format!("{}..{}", chunk_range.start, chunk_range.end - 1)),
            |ui| {
                list_item_ranges(ui, chunk_range, item_fn);
            },
        );
        current = chunk_end;
    }
}
