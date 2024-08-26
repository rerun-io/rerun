use re_chunk_store::external::re_chunk::external::arrow2;
use re_chunk_store::external::re_chunk::external::arrow2::array::Utf8Array;
use re_viewer_context::UiLayout;

//TODO(ab): this is copied/modified from `re_data_ui`. Consider unifying them?
pub(crate) fn arrow_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    array: &dyn arrow2::array::Array,
) -> egui::Response {
    use re_types::SizeBytes as _;

    // Special-treat text.
    // Note: we match on the raw data here, so this works for any component containing text.
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i32>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            return ui_layout.data_label(ui, string);
        }
    }
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i64>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            return ui_layout.data_label(ui, string);
        }
    }

    let num_bytes = array.total_size_bytes();
    if num_bytes < 3000 {
        if array.is_empty() {
            return ui_layout.data_label(ui, "[]");
        }

        // Print small items:
        let mut string = String::new();
        let display = arrow2::array::get_display(array, "null");
        if display(&mut string, 0).is_ok() {
            return ui_layout.data_label(ui, &string);
        }
    }

    // Fallback:
    let bytes = re_format::format_bytes(num_bytes as _);

    let data_type_formatted = format!("{:?}", array.data_type());

    if data_type_formatted.len() < 20 {
        // e.g. "4.2 KiB of Float32"
        ui_layout.data_label(ui, &format!("{bytes} of {data_type_formatted}"))
    } else {
        // Huge datatype, probably a union horror show
        ui_layout.label(ui, format!("{bytes} of data"))
    }
}
