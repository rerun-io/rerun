use arrow::datatypes::DataType;
use egui::Id;
use re_ui::UiExt;
use re_ui::list_item::PropertyContent;

pub fn datatype_ui<'a>(
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
            let label = crate::arrow_ui::simple_datatype_string(non_nested)
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
