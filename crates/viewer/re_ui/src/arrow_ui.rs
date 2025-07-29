use arrow::{
    array::{Array, StructArray},
    datatypes::DataType,
    error::ArrowError,
    util::display::{ArrayFormatter, FormatOptions},
};
use egui::Id;
use itertools::Itertools as _;

use re_arrow_util::ArrowArrayDowncastRef as _;

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

            array_contents_ui(ui, array);

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

fn array_contents_ui(ui: &mut egui::Ui, array: &dyn arrow::array::Array) {
    if array.is_empty() {
        return;
    }

    let array_len = array.len();
    let instance_count_str = re_format::format_uint(array_len);
    let contents_label = format!("{instance_count_str} items");

    list_item_scope(ui, Id::new("array contents list"), |ui| {
        ui.list_item().show_hierarchical_with_children(
            ui,
            Id::new("array contents"),
            true,
            LabelContent::new(contents_label),
            |ui| {
                render_array_items(ui, array);
            },
        );
    });
}

fn render_array_items(ui: &mut egui::Ui, array: &dyn arrow::array::Array) {
    const RANGE_SIZE: usize = 100;
    const MAX_RANGES_TO_SHOW: usize = 10;
    const MAX_ITEMS_PER_RANGE: usize = 10;

    // Handle different array types with custom UI
    match array.data_type() {
        DataType::Struct(_) => {
            render_struct_array_items(
                ui,
                array,
                RANGE_SIZE,
                MAX_RANGES_TO_SHOW,
                MAX_ITEMS_PER_RANGE,
            );
        }
        DataType::List(_)
        | DataType::LargeList(_)
        | DataType::ListView(_)
        | DataType::LargeListView(_)
        | DataType::FixedSizeList(_, _) => {
            render_list_array_items(
                ui,
                array,
                RANGE_SIZE,
                MAX_RANGES_TO_SHOW,
                MAX_ITEMS_PER_RANGE,
            );
        }
        DataType::Union(_, _) => {
            render_union_array_items(
                ui,
                array,
                RANGE_SIZE,
                MAX_RANGES_TO_SHOW,
                MAX_ITEMS_PER_RANGE,
            );
        }
        DataType::Map(_, _) => {
            render_map_array_items(
                ui,
                array,
                RANGE_SIZE,
                MAX_RANGES_TO_SHOW,
                MAX_ITEMS_PER_RANGE,
            );
        }
        _ => {
            // Primitive types and other simple types
            render_primitive_array_items(
                ui,
                array,
                RANGE_SIZE,
                MAX_RANGES_TO_SHOW,
                MAX_ITEMS_PER_RANGE,
            );
        }
    }
}

fn render_struct_array_items(
    ui: &mut egui::Ui,
    array: &dyn arrow::array::Array,
    range_size: usize,
    max_ranges: usize,
    max_items_per_range: usize,
) {
    let Some(struct_array) = array.as_any().downcast_ref::<StructArray>() else {
        return;
    };
    let array_len = struct_array.len();

    if array_len <= max_items_per_range {
        // Show all items individually for small arrays
        for i in 0..array_len {
            render_struct_item(ui, struct_array, i);
        }
    } else {
        // Group items into ranges for large arrays
        let mut ranges_shown = 0;
        let mut start = 0;

        while start < array_len && ranges_shown < max_ranges {
            let end = (start + range_size).min(array_len);
            let range_label = if end - start == 1 {
                format!("[{start}]")
            } else {
                format!("[{start}..{}]", end - 1)
            };

            let range_content =
                PropertyContent::new(range_label.clone()).show_only_when_collapsed(false);

            ui.list_item().show_hierarchical_with_children(
                ui,
                Id::new(format!("struct_range_{start}_{end}")),
                false, // collapsed by default
                range_content,
                |ui| {
                    let items_to_show = (end - start).min(max_items_per_range);
                    for i in start..(start + items_to_show) {
                        render_struct_item(ui, struct_array, i);
                    }

                    if end - start > max_items_per_range {
                        let omitted = end - start - max_items_per_range;
                        let content =
                            PropertyContent::new("...").value_text(format!("{omitted} more items"));
                        ui.list_item().show_flat(ui, content);
                    }
                },
            );

            start = end;
            ranges_shown += 1;
        }

        if start < array_len {
            let remaining = array_len - start;
            let content =
                PropertyContent::new("...").value_text(format!("{remaining} more ranges"));
            ui.list_item().show_flat(ui, content);
        }
    }
}

fn render_struct_item(ui: &mut egui::Ui, struct_array: &StructArray, index: usize) {
    let struct_label = format!("[{index}]");

    ui.list_item().show_hierarchical_with_children(
        ui,
        Id::new(format!("struct_item_{index}")),
        false, // collapsed by default
        PropertyContent::new(struct_label).show_only_when_collapsed(false),
        |ui| {
            // Show each field of the struct
            for (field_index, _field) in struct_array.fields().iter().enumerate() {
                let field_array = struct_array.column(field_index);
                let field_name = struct_array.column_names()[field_index];

                match field_array.data_type() {
                    // For nested types, show hierarchically
                    DataType::Struct(_)
                    | DataType::List(_)
                    | DataType::LargeList(_)
                    | DataType::ListView(_)
                    | DataType::LargeListView(_)
                    | DataType::FixedSizeList(_, _)
                    | DataType::Union(_, _)
                    | DataType::Map(_, _) => {
                        ui.list_item().show_hierarchical_with_children(
                            ui,
                            Id::new(format!("struct_field_{index}_{field_index}")),
                            false,
                            PropertyContent::new(field_name).show_only_when_collapsed(false),
                            |ui| {
                                // Recursively render the nested field
                                let single_item_array = field_array.slice(index, 1);
                                arrow_ui(ui, UiLayout::SelectionPanel, &single_item_array);
                            },
                        );
                    }
                    _ => {
                        // For primitive types, show the value directly
                        if let Ok(formatter) = make_formatter(field_array.as_ref()) {
                            let content =
                                PropertyContent::new(field_name).value_text(formatter(index));
                            ui.list_item().show_flat(ui, content);
                        }
                    }
                }
            }
        },
    );
}

fn render_list_array_items(
    ui: &mut egui::Ui,
    array: &dyn arrow::array::Array,
    range_size: usize,
    max_ranges: usize,
    max_items_per_range: usize,
) {
    let array_len = array.len();

    if array_len <= max_items_per_range {
        for i in 0..array_len {
            render_list_item(ui, array, i);
        }
    } else {
        // Group items into ranges for large arrays
        let mut ranges_shown = 0;
        let mut start = 0;

        while start < array_len && ranges_shown < max_ranges {
            let end = (start + range_size).min(array_len);
            let range_label = if end - start == 1 {
                format!("[{start}]")
            } else {
                format!("[{start}..{}]", end - 1)
            };

            let range_content =
                PropertyContent::new(range_label.clone()).show_only_when_collapsed(false);

            ui.list_item().show_hierarchical_with_children(
                ui,
                Id::new(format!("list_range_{start}_{end}")),
                false,
                range_content,
                |ui| {
                    let items_to_show = (end - start).min(max_items_per_range);
                    for i in start..(start + items_to_show) {
                        render_list_item(ui, array, i);
                    }

                    if end - start > max_items_per_range {
                        let omitted = end - start - max_items_per_range;
                        let content =
                            PropertyContent::new("...").value_text(format!("{omitted} more items"));
                        ui.list_item().show_flat(ui, content);
                    }
                },
            );

            start = end;
            ranges_shown += 1;
        }

        if start < array_len {
            let remaining = array_len - start;
            let content =
                PropertyContent::new("...").value_text(format!("{remaining} more ranges"));
            ui.list_item().show_flat(ui, content);
        }
    }
}

fn render_list_item(ui: &mut egui::Ui, array: &dyn arrow::array::Array, index: usize) {
    use arrow::array::{LargeListArray, ListArray};

    let list_label = format!("[{index}]");

    ui.list_item().show_hierarchical_with_children(
        ui,
        Id::new(format!("list_item_{index}")),
        false,
        PropertyContent::new(list_label).show_only_when_collapsed(false),
        |ui| {
            // Get the list value at this index and recursively render it
            if let Some(list_array) = array.as_any().downcast_ref::<ListArray>() {
                let list_value = list_array.value(index);
                arrow_ui(ui, UiLayout::SelectionPanel, &list_value);
            } else if let Some(large_list_array) = array.as_any().downcast_ref::<LargeListArray>() {
                let list_value = large_list_array.value(index);
                arrow_ui(ui, UiLayout::SelectionPanel, &list_value);
            }
        },
    );
}

fn render_union_array_items(
    ui: &mut egui::Ui,
    array: &dyn arrow::array::Array,
    range_size: usize,
    max_ranges: usize,
    max_items_per_range: usize,
) {
    // For now, treat unions like primitive arrays
    // TODO: Add proper union handling
    render_primitive_array_items(ui, array, range_size, max_ranges, max_items_per_range);
}

fn render_map_array_items(
    ui: &mut egui::Ui,
    array: &dyn arrow::array::Array,
    range_size: usize,
    max_ranges: usize,
    max_items_per_range: usize,
) {
    // For now, treat maps like primitive arrays
    // TODO: Add proper map handling
    render_primitive_array_items(ui, array, range_size, max_ranges, max_items_per_range);
}

fn render_primitive_array_items(
    ui: &mut egui::Ui,
    array: &dyn arrow::array::Array,
    range_size: usize,
    max_ranges: usize,
    max_items_per_range: usize,
) {
    let array_len = array.len();
    let Ok(array_formatter) = make_formatter(array) else {
        return;
    };

    if array_len <= max_items_per_range {
        // Show all items individually for small arrays
        for i in 0..array_len {
            let content = PropertyContent::new(format!("[{i}]")).value_text(array_formatter(i));
            ui.list_item().show_flat(ui, content);
        }
    } else {
        // Group items into ranges for large arrays
        let mut ranges_shown = 0;
        let mut start = 0;

        while start < array_len && ranges_shown < max_ranges {
            let end = (start + range_size).min(array_len);
            let range_label = if end - start == 1 {
                format!("[{start}]")
            } else {
                format!("[{start}..{}]", end - 1)
            };

            let range_content =
                PropertyContent::new(range_label.clone()).show_only_when_collapsed(false);

            ui.list_item().show_hierarchical_with_children(
                ui,
                Id::new(format!("primitive_range_{start}_{end}")),
                false, // collapsed by default
                range_content,
                |ui| {
                    let items_to_show = (end - start).min(max_items_per_range);
                    for i in start..(start + items_to_show) {
                        let content =
                            PropertyContent::new(format!("[{i}]")).value_text(array_formatter(i));
                        ui.list_item().show_flat(ui, content);
                    }

                    if end - start > max_items_per_range {
                        let omitted = end - start - max_items_per_range;
                        let content =
                            PropertyContent::new("...").value_text(format!("{omitted} more items"));
                        ui.list_item().show_flat(ui, content);
                    }
                },
            );

            start = end;
            ranges_shown += 1;
        }

        if start < array_len {
            let remaining = array_len - start;
            let content =
                PropertyContent::new("...").value_text(format!("{remaining} more ranges"));
            ui.list_item().show_flat(ui, content);
        }
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
