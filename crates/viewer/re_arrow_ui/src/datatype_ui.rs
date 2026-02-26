use arrow::datatypes::DataType;
use re_ui::UiExt as _;
use re_ui::list_item::PropertyContent;

/// Show an ui describing an Arrow `DataType`.
///
/// This is basically a string type name and an optional closure describing nested fields.
pub struct DataTypeUi<'a> {
    /// A short name for the type, e.g. "struct", "list", "int32", etc.
    ///
    /// Doesn't contain info about nested fields.
    pub type_name: String,

    /// A closure for showing a list item with info about nested fields.
    ///
    /// Only set if the type actually has nested fields.
    #[expect(clippy::type_complexity)]
    pub content: Option<Box<dyn FnOnce(&mut egui::Ui) + 'a>>,
}

impl<'a> DataTypeUi<'a> {
    pub fn new(data_type: &'a DataType) -> Self {
        match data_type {
            DataType::Struct(fields) => DataTypeUi {
                type_name: "struct".to_owned(),
                content: Some(Box::new(move |ui| {
                    for field in fields {
                        data_type_field_ui(ui, field);
                    }
                })),
            },
            DataType::List(field) => DataTypeUi {
                type_name: "list".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::ListView(field) => DataTypeUi {
                type_name: "list view".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::FixedSizeList(field, size) => DataTypeUi {
                type_name: format!("fixed-size list ({size})"),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::LargeList(field) => DataTypeUi {
                type_name: "large list".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::LargeListView(field) => DataTypeUi {
                type_name: "large list view".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::Union(fields, mode) => {
                let label = match mode {
                    arrow::datatypes::UnionMode::Sparse => "sparse union",
                    arrow::datatypes::UnionMode::Dense => "dense union",
                };
                DataTypeUi {
                    type_name: label.to_owned(),
                    content: Some(Box::new(move |ui| {
                        for (_, field) in fields.iter() {
                            data_type_field_ui(ui, field);
                        }
                    })),
                }
            }
            DataType::Dictionary(_k, _v) => DataTypeUi {
                type_name: "dictionary".to_owned(),
                content: None,
            },
            DataType::Map(a, _) => DataTypeUi {
                type_name: "map".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, a);
                })),
            },
            DataType::RunEndEncoded(a, b) => DataTypeUi {
                type_name: "run-end encoded".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, a);
                    data_type_field_ui(ui, b);
                })),
            },
            non_nested => {
                let label = simple_data_type_string(non_nested)
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| non_nested.to_string());
                DataTypeUi {
                    type_name: label,
                    content: None,
                }
            }
        }
    }

    /// Show the data type as a `list_item`.
    ///
    /// The root item has the label "Data type".
    pub fn list_item_ui(self, ui: &mut egui::Ui) {
        let content = PropertyContent::new("Data type")
            .value_text(self.type_name)
            .show_only_when_collapsed(false);
        if let Some(datatype_ui) = self.content {
            ui.list_item().show_hierarchical_with_children(
                ui,
                ui.id().with("data_type_ui_root"),
                false,
                content,
                datatype_ui,
            );
        } else {
            ui.list_item().show_hierarchical(ui, content);
        }
    }
}

fn data_type_field_ui(ui: &mut egui::Ui, field: &arrow::datatypes::Field) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let item = ui.list_item();

    let data_type_ui = DataTypeUi::new(field.data_type());

    let text = if field.is_nullable() {
        format!("nullable {}", data_type_ui.type_name)
    } else {
        field.name().to_owned()
    };

    let property = PropertyContent::new(field.name())
        .value_text(text)
        .show_only_when_collapsed(false);

    if let Some(content) = data_type_ui.content {
        item.show_hierarchical_with_children(
            ui,
            ui.unique_id().with(field.name()),
            true,
            property,
            content,
        );
    } else {
        item.show_hierarchical(ui, property);
    }
}

// TODO(#11071): there is some overlap here with `re_arrow_util::format` and `codegen`.
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
