use re_types::external::arrow2::{self, array::Utf8Array};
use re_viewer_context::{
    external::{
        re_chunk_store::{LatestAtQuery, RowId},
        re_entity_db::EntityDb,
        re_log_types::EntityPath,
    },
    UiLayout, ViewerContext,
};

#[allow(clippy::too_many_arguments)]
pub fn fallback_component_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _entity_path: &EntityPath,
    _row_id: Option<RowId>,
    component: &dyn arrow::array::Array,
) {
    arrow_ui(ui, ui_layout, component);
}

fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow::array::Array) {
    arrow2_ui(
        ui,
        ui_layout,
        Box::<dyn arrow2::array::Array>::from(array).as_ref(),
    );
}

fn arrow2_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow2::array::Array) {
    use re_byte_size::SizeBytes as _;

    // Special-treat text.
    // Note: we match on the raw data here, so this works for any component containing text.
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i32>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            ui_layout.data_label(ui, string);
            return;
        }
    }
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i64>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            ui_layout.data_label(ui, string);
            return;
        }
    }

    let num_bytes = array.total_size_bytes();
    if num_bytes < 3000 {
        // Print small items:
        let mut string = String::new();
        let display = arrow2::array::get_display(array, "null");
        if display(&mut string, 0).is_ok() {
            ui_layout.data_label(ui, &string);
            return;
        }
    }

    // Fallback:
    let bytes = re_format::format_bytes(num_bytes as _);

    // TODO(emilk): pretty-print data type
    let data_type_formatted = format!("{:?}", array.data_type());

    if data_type_formatted.len() < 20 {
        // e.g. "4.2 KiB of Float32"
        ui_layout.data_label(ui, format!("{bytes} of {data_type_formatted}"));
    } else {
        // Huge datatype, probably a union horror show
        ui.label(format!("{bytes} of data"));
    }
}
