use arrow::array::AsArray;
use arrow::{
    array::{Array, StructArray},
    datatypes::DataType,
    error::ArrowError,
    util::display::{ArrayFormatter, FormatOptions},
};
use egui::{Id, Response, Ui, Widget, WidgetText};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::list_item::{LabelContent, PropertyContent, list_item_scope};
use crate::{UiExt, UiLayout};

pub fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow::array::Array) {
    re_tracing::profile_function!();

    use arrow::array::{LargeListArray, LargeStringArray, ListArray, StringArray};

    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        if ui_layout.is_selection_panel() {
            let (datatype_name, maybe_ui) = datatype_ui(array.data_type());
            if let Some(content) = maybe_ui {
                list_item_scope(ui, Id::new("arrow data type list"), |ui| {
                    ui.list_item().show_hierarchical_with_children(
                        ui,
                        Id::new("arrow data type"),
                        true,
                        LabelContent::new(datatype_name),
                        content,
                    );
                });
            }

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
            "struct".to_string(),
            Some(Box::new(move |ui| {
                for field in fields {
                    datatype_field_ui(ui, field);
                }
            })),
        ),
        DataType::List(field) => (
            "list".to_string(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, field);
            })),
        ),
        DataType::ListView(field) => (
            "list view".to_string(),
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
            "large list".to_string(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, field);
            })),
        ),
        DataType::LargeListView(field) => (
            "large list view".to_string(),
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
                label.to_string(),
                Some(Box::new(move |ui| {
                    for (_, field) in fields.iter() {
                        datatype_field_ui(ui, field);
                    }
                })),
            )
        }
        DataType::Dictionary(_k, _v) => (
            format!("dictionary"),
            // Some(Box::new(move |ui| {
            //     ui.list_item()
            //         .show_flat(ui, PropertyContent::new("key").value_text(k.to_string()));
            //     ui.list_item()
            //         .show_flat(ui, PropertyContent::new("value").value_text(v.to_string()));
            // })),
            None, // TODO
        ),
        DataType::Map(a, _) => (
            "map".to_string(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, a);
            })),
        ),
        DataType::RunEndEncoded(a, b) => (
            "run-end encoded".to_string(),
            Some(Box::new(move |ui| {
                datatype_field_ui(ui, a);
                datatype_field_ui(ui, b);
            })),
        ),
        non_nested => {
            let label = simple_datatype_string(non_nested)
                .map(|s| s.to_string())
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
    for i in 0..array.len() {
        let node = array_item_ui(array, i);

        node.ui(ui, format!("[{i}]"));
    }
}

fn array_item_ui<'a>(array: &'a dyn Array, index: usize) -> ArrowNode<'a> {
    let formatter = make_formatter(array).expect("Formatter should be created");
    let value = formatter(index);

    let mut node = ArrowNode::new(value, array.data_type());

    if let Some(struct_array) = array.as_struct_opt() {
        node = node.with_children(move |ui| {
            for column_name in struct_array.column_names() {
                let column = struct_array
                    .column_by_name(column_name)
                    .expect("Field should exist");

                let node = array_item_ui(column.as_ref(), index);
                node.ui(ui, column_name);
            }
        });
    } else if let Some(list) = array.as_list_opt::<i32>() {
        node = node.with_children(move |ui| {
            let value = list.value(index);
            array_items_ui(ui, &value);
        });
    } else if let Some(list) = array.as_list_opt::<i64>() {
        node = node.with_children(move |ui| {
            let value = list.value(index);
            array_items_ui(ui, &value);
        });
    } else if let Some(list_array) = array.as_fixed_size_list_opt() {
        node = node.with_children(move |ui| {
            let value = list_array.value(index);
            array_items_ui(ui, &value);
        });
    } else if let Some(dict_array) = array.as_any_dictionary_opt() {
        node = node.with_children(move |ui| {
            if !dict_array.keys().data_type().is_nested() {
                let formatter = make_formatter(dict_array.keys())
                    .expect("Formatter should be created for dictionary keys");
                let key_string = formatter(index);
                let node = array_item_ui(dict_array.values().as_ref(), index);
                node.ui(ui, key_string);
            } else {
                let key_node = array_item_ui(dict_array.keys(), index);
                let value_node = array_item_ui(dict_array.values().as_ref(), index);
                key_node.ui(ui, "key");
                value_node.ui(ui, "value");
            }
        });
    } else if let Some(map_array) = array.as_map_opt() {
        node = node.with_children(move |ui| {
            if !map_array.keys().data_type().is_nested() {
                let formatter = make_formatter(map_array.keys())
                    .expect("Formatter should be created for map keys");
                let key_string = formatter(index);
                let node = array_item_ui(map_array.values().as_ref(), index);
                node.ui(ui, key_string);
            } else {
                let key_node = array_item_ui(map_array.keys().as_ref(), index);
                let value_node = array_item_ui(map_array.values().as_ref(), index);
                key_node.ui(ui, "key");
                value_node.ui(ui, "value");
            }
        });
    } else if let Some(union_array) = array.as_union_opt() {
        let variant_index = union_array.type_id(index);
        let child = union_array.child(variant_index);
        let node = array_item_ui(child, index);
        return node;
    }

    node
}

struct ArrowNode<'a> {
    value: String,
    data_type: &'a DataType,
    children: Option<Box<dyn FnOnce(&mut egui::Ui) + 'a>>,
}

impl<'a> ArrowNode<'a> {
    fn new(value: String, data_type: &'a DataType) -> Self {
        ArrowNode {
            value,
            data_type,
            children: None,
        }
    }

    fn with_children<F: FnOnce(&mut egui::Ui) + 'a>(mut self, children: F) -> Self {
        self.children = Some(Box::new(children));
        self
    }

    fn ui(self, ui: &mut Ui, name: impl Into<WidgetText>) -> Response {
        let ArrowNode {
            value,
            data_type,
            children: maybe_children,
        } = self;

        let data_type_name = datatype_ui(self.data_type).0;

        let item = ui.list_item();

        let text = name.into();
        let id = ui.unique_id().with(&text.text());
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
                            ui.monospace(value);
                        },
                        |ui| {
                            if visuals.hovered {
                                ui.weak(data_type_name);
                            }
                        },
                    );
                });
            })
            .show_only_when_collapsed(false);

        let response = if let Some(children) = maybe_children {
            item.show_hierarchical_with_children(ui, id, false, content, |ui| {
                // We create a new scope so properties are only aligned on a single level
                ui.list_item_scope(id.with("scope"), |ui| {
                    children(ui);
                });
            })
            .item_response
        } else {
            item.show_hierarchical(ui, content)
        };

        response
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
