use crate::child_nodes;
use arrow::array::AsArray;
use arrow::{
    array::Array,
    datatypes::DataType,
    error::ArrowError,
    util::display::{ArrayFormatter, FormatOptions},
};
use egui::text::LayoutJob;
use egui::{Id, Response, RichText, Stroke, StrokeKind, TextFormat, TextStyle, Ui, WidgetText};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_ui::list_item::{LabelContent, PropertyContent, list_item_scope};
use re_ui::{UiExt, UiLayout};
use std::ops::Range;

pub fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow::array::Array) {
    re_tracing::profile_function!();

    use arrow::array::{LargeListArray, LargeStringArray, ListArray, StringArray};

    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        if ui_layout.is_selection_panel() {
            list_item_scope(ui, Id::new("arrow data"), |ui| {
                array_items_ui(ui, array);
            });

            return;
        }

        if array.is_empty() {
            ui_layout.data_label(ui, "[]");
            return;
        }

        // Special-treat text.
        // This is so that we can show urls as clickable links.
        // Note: we match on the raw data here, so this works for any component containing text.
        if let Some(entries) = array.downcast_array_ref::<StringArray>() {
            if entries.len() == 1 {
                let string = entries.value(0);
                ui_layout.data_label(ui, string);
                return;
            }
        }
        if let Some(entries) = array.downcast_array_ref::<LargeStringArray>() {
            if entries.len() == 1 {
                let string = entries.value(0);
                ui_layout.data_label(ui, string);
                return;
            }
        }

        // Special-treat batches that are themselves unit-lists (i.e. blobs).
        //
        // What we really want to display in these instances in the underlying array, otherwise we'll
        // bring down the entire viewer trying to render a list whose single entry might itself be
        // an array with millions of values.
        if let Some(entries) = array.downcast_array_ref::<ListArray>() {
            if entries.len() == 1 {
                // Don't use `values` since this may contain values before and after the single blob we want to display.
                return arrow_ui(ui, ui_layout, &entries.value(0));
            }
        }
        if let Some(entries) = array.downcast_array_ref::<LargeListArray>() {
            if entries.len() == 1 {
                // Don't use `values` since this may contain values before and after the single blob we want to display.
                return arrow_ui(ui, ui_layout, &entries.value(0));
            }
        }

        let Ok(array_formatter) = make_formatter(array) else {
            // This is unreachable because we use `.with_display_error(true)` above.
            return;
        };

        let instance_count = array.len();

        if instance_count == 1 {
            ui_layout.data_label(ui, array_formatter(0));
        } else if instance_count < 10
            && (array.data_type().is_primitive()
                || matches!(array.data_type(), DataType::Utf8 | DataType::LargeUtf8))
        {
            // A short list of floats, strings, etc. Show it to the user.
            let list_string = format!("[{}]", (0..instance_count).map(array_formatter).join(", "));
            ui_layout.data_label(ui, list_string);
        } else {
            let instance_count_str = re_format::format_uint(instance_count);

            let string = if array.data_type() == &DataType::UInt8 {
                re_format::format_bytes(instance_count as _)
            } else if let Some(dtype) = simple_datatype_string(array.data_type()) {
                format!("{instance_count_str} items of {dtype}")
            } else if let DataType::Struct(fields) = array.data_type() {
                format!(
                    "{instance_count_str} structs with {} fields: {}",
                    fields.len(),
                    fields
                        .iter()
                        .map(|f| format!("{}:{}", f.name(), f.data_type()))
                        .join(", ")
                )
            } else {
                format!("{instance_count_str} items")
            };
            ui_layout.label(ui, string).on_hover_ui(|ui| {
                const MAX_INSTANCE: usize = 40;

                let list_string = format!(
                    "[{}{}]{}",
                    (0..instance_count.min(MAX_INSTANCE))
                        .map(array_formatter)
                        .join(", "),
                    if instance_count > MAX_INSTANCE {
                        ", …"
                    } else {
                        ""
                    },
                    if instance_count > MAX_INSTANCE {
                        format!(" ({} items omitted)", instance_count - MAX_INSTANCE)
                    } else {
                        String::new()
                    }
                );

                UiLayout::Tooltip.data_label(ui, list_string);
            });
        }
    });
}

fn datatype_ui<'a>(
    data_type: &'a DataType,
) -> (String, Option<Box<dyn FnOnce(&mut egui::Ui) + 'a>>) {
    match data_type {
        DataType::Struct(fields) => (
            "struct".to_owned(),
            Some(Box::new(move |ui| {
                for field in fields {
                    datatype_field_ui(ui, field);
                }
            })),
        ),
        DataType::List(field) => (
            "list".to_owned(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, field);
            })),
        ),
        DataType::ListView(field) => (
            "list view".to_owned(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, field);
            })),
        ),
        DataType::FixedSizeList(field, size) => (
            format!("fixed-size list ({size})"),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, field);
            })),
        ),
        DataType::LargeList(field) => (
            "large list".to_owned(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, field);
            })),
        ),
        DataType::LargeListView(field) => (
            "large list view".to_owned(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, field);
            })),
        ),
        DataType::Union(fields, mode) => {
            let label = match mode {
                arrow::datatypes::UnionMode::Sparse => "sparse union",
                arrow::datatypes::UnionMode::Dense => "dense union",
            };
            (
                label.to_owned(),
                Some(Box::new(move |ui| {
                    for (_, field) in fields.iter() {
                        datatype_field_ui(ui, field);
                    }
                })),
            )
        }
        DataType::Dictionary(_k, _v) => (
            "dictionary".to_owned(),
            // Some(Box::new(move |ui| {
            //     ui.list_item()
            //         .show_flat(ui, PropertyContent::new("key").value_text(k.to_string()));
            //     ui.list_item()
            //         .show_flat(ui, PropertyContent::new("value").value_text(v.to_string()));
            // })),
            None, // TODO
        ),
        DataType::Map(a, _) => (
            "map".to_owned(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, a);
            })),
        ),
        DataType::RunEndEncoded(a, b) => (
            "run-end encoded".to_owned(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, a);
                datatype_field_ui(ui, b);
            })),
        ),
        non_nested => {
            let label = simple_datatype_string(non_nested)
                .map(|s| s.to_owned())
                .unwrap_or_else(|| non_nested.to_string());
            (label, None)
        }
    }
}

fn datatype_field_ui(ui: &mut egui::Ui, field: &arrow::datatypes::Field) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let item = ui.list_item();

    let (datatype_name, maybe_ui) = datatype_ui(field.data_type());

    let property = PropertyContent::new(field.name())
        .value_text(datatype_name)
        .show_only_when_collapsed(false);

    if let Some(content) = maybe_ui {
        item.show_hierarchical_with_children(ui, Id::new(field.name()), true, property, content);
    } else {
        item.show_hierarchical(ui, property);
    }
}

fn array_items_ui(ui: &mut egui::Ui, array: &dyn Array) {
    dbg!(array.len(), array.data_type());
    for i in 0..array.len() {
        let node = ArrowNode::new(array, i);
        node.ui(ui);
    }
}

pub struct ArrowNode<'a> {
    array: child_nodes::MaybeArc<'a>,
    index: usize,
    field_name: Option<WidgetText>, // TODO: Can be &str?
}

impl<'a> ArrowNode<'a> {
    pub fn new(array: impl Into<child_nodes::MaybeArc<'a>>, index: usize) -> Self {
        ArrowNode {
            array: array.into(),
            index,
            field_name: None,
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

        if let Some(children) = child_nodes::ChildNodes::new(self.array.as_ref(), self.index) {
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
            let mut value = if let Ok(formatter) = make_formatter(self.array.as_ref()) {
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

    fn ui(self, ui: &mut Ui) -> Response {
        let ArrowNode {
            array,
            index,
            field_name,
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

        let maybe_children = child_nodes::ChildNodes::new(array, *index);
        let response = if let Some(children) = maybe_children {
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

fn make_formatter(array: &dyn Array) -> Result<Box<dyn Fn(usize) -> String + '_>, ArrowError> {
    // It would be nice to add quotes around strings,
    // but we already special-case single strings so that we can show them as links,
    // so we if we change things here we need to change that too. Maybe we should.
    let options = FormatOptions::default()
        .with_null("null")
        .with_display_error(true);
    let formatter = ArrayFormatter::try_new(array, &options)?;
    Ok(Box::new(move |index| formatter.value(index).to_string()))
}

// TODO(emilk): there is some overlap here with `re_format_arrow`.
fn simple_datatype_string(datatype: &DataType) -> Option<&'static str> {
    match datatype {
        DataType::Null => Some("null"),
        DataType::Boolean => Some("bool"),
        DataType::Int8 => Some("int8"),
        DataType::Int16 => Some("int16"),
        DataType::Int32 => Some("int32"),
        DataType::Int64 => Some("int64"),
        DataType::UInt8 => Some("uint8"),
        DataType::UInt16 => Some("uint16"),
        DataType::UInt32 => Some("uint32"),
        DataType::UInt64 => Some("uint64"),
        DataType::Float16 => Some("float16"),
        DataType::Float32 => Some("float32"),
        DataType::Float64 => Some("float64"),
        DataType::Utf8 | DataType::LargeUtf8 => Some("utf8"),
        DataType::Binary | DataType::LargeBinary => Some("binary"),
        _ => None,
    }
}
