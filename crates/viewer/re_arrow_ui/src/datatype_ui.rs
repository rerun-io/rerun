use arrow::datatypes::DataType;
use egui::Id;
use re_ui::UiExt;
use re_ui::list_item::PropertyContent;

pub fn data_type_ui<'a>(
    data_type: &'a DataType,
) -> (String, Option<Box<dyn FnOnce(&mut egui::Ui) + 'a>>) {
    match data_type {
        DataType::Struct(fields) => (
            "struct".to_owned(),
            Some(Box::new(move |ui| {
                for field in fields {
                    data_type_field_ui(ui, field);
                }
            })),
        ),
        DataType::List(field) => (
            "list".to_owned(),
            Some(Box::new(move |ui| {
                data_type_field_ui(ui, field);
            })),
        ),
        DataType::ListView(field) => (
            "list view".to_owned(),
            Some(Box::new(move |ui| {
                data_type_field_ui(ui, field);
            })),
        ),
        DataType::FixedSizeList(field, size) => (
            format!("fixed-size list ({size})"),
            Some(Box::new(move |ui| {
                data_type_field_ui(ui, field);
            })),
        ),
        DataType::LargeList(field) => (
            "large list".to_owned(),
            Some(Box::new(move |ui| {
                data_type_field_ui(ui, field);
            })),
        ),
        DataType::LargeListView(field) => (
            "large list view".to_owned(),
            Some(Box::new(move |ui| {
                data_type_field_ui(ui, field);
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
                        data_type_field_ui(ui, field);
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
                data_type_field_ui(ui, a);
            })),
        ),
        DataType::RunEndEncoded(a, b) => (
            "run-end encoded".to_owned(),
            Some(Box::new(move |ui| {
                data_type_field_ui(ui, a);
                data_type_field_ui(ui, b);
            })),
        ),
        non_nested => {
            let label = simple_data_type_string(non_nested)
                .map(|s| s.to_owned())
                .unwrap_or_else(|| non_nested.to_string());
            (label, None)
        }
    }
}

fn data_type_field_ui(ui: &mut egui::Ui, field: &arrow::datatypes::Field) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let item = ui.list_item();

    let (datatype_name, maybe_ui) = data_type_ui(field.data_type());

    let property = PropertyContent::new(field.name())
        .value_text(datatype_name)
        .show_only_when_collapsed(false);

    if let Some(content) = maybe_ui {
        item.show_hierarchical_with_children(ui, Id::new(field.name()), true, property, content);
    } else {
        item.show_hierarchical(ui, property);
    }
}

// TODO(emilk): there is some overlap here with `re_format_arrow`.
pub(crate) fn simple_data_type_string(datatype: &DataType) -> Option<&'static str> {
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
