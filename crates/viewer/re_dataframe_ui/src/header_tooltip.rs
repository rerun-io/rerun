use std::collections::BTreeMap;

use arrow::datatypes::Field;
use re_sorbet::{BatchType, ColumnDescriptorRef};

pub fn column_header_tooltip_ui(
    ui: &mut egui::Ui,
    desc: &ColumnDescriptorRef<'_>,
    column_field: &Field,
    migrated_column_field: &Field,
    show_extras: bool,
) {
    column_descriptor_ui(ui, desc, column_field);
    column_arrow_metadata_ui(ui, column_field, show_extras);

    if show_extras {
        extras_column_descriptor_ui(ui, desc, migrated_column_field);
    } else {
        ui.separator();
        ui.weak("Hold `Alt` to see extras");
    }
}

fn column_descriptor_ui(ui: &mut egui::Ui, column: &ColumnDescriptorRef<'_>, column_field: &Field) {
    header_property_ui(ui, "Physical name", column_field.name());

    match *column {
        ColumnDescriptorRef::RowId(desc) => {
            let re_sorbet::RowIdColumnDescriptor { is_sorted } = desc;

            header_property_ui(ui, "Type", "row id");
            header_property_ui(ui, "Sorted", sorted_text(*is_sorted));
        }
        ColumnDescriptorRef::Time(desc) => {
            let re_sorbet::IndexColumnDescriptor {
                timeline,
                datatype,
                is_sorted,
            } = desc;

            header_property_ui(ui, "Type", "index");
            header_property_ui(ui, "Timeline", timeline.name());
            header_property_ui(ui, "Sorted", sorted_text(*is_sorted));
            datatype_ui(ui, &column.display_name(), datatype);
        }
        ColumnDescriptorRef::Component(desc) => {
            let re_sorbet::ComponentColumnDescriptor {
                store_datatype,
                component_type,
                entity_path,
                archetype,
                component,
                is_static: _,
                is_tombstone: _,
                is_semantically_empty: _,
            } = desc;

            header_property_ui(ui, "Column type", "Component");
            header_property_ui(ui, "Entity path", entity_path.to_string());
            datatype_ui(ui, &column.display_name(), store_datatype);
            header_property_ui(
                ui,
                "Archetype",
                archetype.map(|a| a.full_name()).unwrap_or("-"),
            );
            header_property_ui(ui, "Component", component);
            header_property_ui(
                ui,
                "Component type",
                component_type.map(|a| a.as_str()).unwrap_or("-"),
            );
        }
    }
}

fn column_arrow_metadata_ui(ui: &mut egui::Ui, column_field: &Field, show_extras: bool) {
    let user_metadata = column_field
        .metadata()
        .iter()
        .filter(|&(key, _)| !key.starts_with("rerun:"))
        .collect::<BTreeMap<_, _>>();

    let sorbet_metadata = column_field
        .metadata()
        .iter()
        .filter(|&(key, _)| key.starts_with("rerun:"))
        .collect::<BTreeMap<_, _>>();

    // user metadata
    if !user_metadata.is_empty() {
        ui.separator();
        ui.weak("Arrow metadata");
        for (key, value) in user_metadata {
            header_property_ui(ui, key, value);
        }
    }

    // sorbet metadata
    if !sorbet_metadata.is_empty() && show_extras {
        ui.separator();
        ui.weak("Sorbet metadata");
        for (key, value) in sorbet_metadata {
            header_property_ui(ui, key, value);
        }
    }
}

fn extras_column_descriptor_ui(
    ui: &mut egui::Ui,
    column: &ColumnDescriptorRef<'_>,
    migrated_field: &Field,
) {
    ui.separator();
    ui.weak("Extras");

    header_property_ui(ui, "Migrated physical name", migrated_field.name());
    header_property_ui(
        ui,
        "Descriptor column name (chunk mode)",
        column.column_name(BatchType::Chunk),
    );
    header_property_ui(
        ui,
        "Descriptor column name (dataframe mode)",
        column.column_name(BatchType::Dataframe),
    );
    header_property_ui(ui, "Descriptor display name", column.display_name());

    match *column {
        ColumnDescriptorRef::Component(desc) => {
            // TODO(#10315): these are sometimes inaccurate.
            header_property_ui(ui, "Is static", desc.is_static.to_string());
            header_property_ui(ui, "Is tombstone", desc.is_tombstone.to_string());
            header_property_ui(ui, "Is empty", desc.is_semantically_empty.to_string());
        }
        ColumnDescriptorRef::RowId(_) | ColumnDescriptorRef::Time(_) => {}
    }
}

fn sorted_text(sorted: bool) -> &'static str {
    if sorted { "true" } else { "unknown" }
}

fn header_property_ui(ui: &mut egui::Ui, label: &str, value: impl AsRef<str>) {
    egui::Sides::new().show(ui, |ui| ui.strong(label), |ui| ui.monospace(value.as_ref()));
}

fn datatype_ui(ui: &mut egui::Ui, column_name: &str, datatype: &arrow::datatypes::DataType) {
    egui::Sides::new().show(
        ui,
        |ui| ui.strong("Datatype"),
        |ui| {
            // We don't want the copy button to stand out next to the other properties. The copy
            // icon already indicates that it's a button.
            ui.visuals_mut().widgets.inactive.fg_stroke =
                ui.visuals_mut().widgets.noninteractive.fg_stroke;

            if ui
                .add(
                    egui::Button::image_and_text(
                        re_ui::icons::COPY.as_image(),
                        // TODO(#11071): use re_arrow_ui to format the datatype here
                        egui::RichText::new(re_arrow_util::format_data_type(datatype)).monospace(),
                    )
                    .image_tint_follows_text_color(true),
                )
                .clicked()
            {
                ui.ctx().copy_text(format!("{datatype:#?}")); // TODO(apache/arrow-rs#8351): use Display once arrow 57 is released
                re_log::info!("Copied full datatype of column `{column_name}` to clipboard");
            }
        },
    );
}
