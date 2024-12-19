use itertools::Itertools;

use re_byte_size::SizeBytes as _;
use re_chunk_store::external::arrow2;
use re_chunk_store::external::arrow2::array::Utf8Array;
use re_ui::UiExt;

// Note: this is copied and heavily modified from `re_data_ui`. We don't want to unify them because
// that would likely introduce an undesired dependency (`re_chunk_store_ui` should remain as
// independent as possible from the viewer, so it may be split off one day).
pub(crate) fn arrow2_ui(ui: &mut egui::Ui, array: &dyn arrow2::array::Array) {
    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

        // Special-treat text.
        // Note: we match on the raw data here, so this works for any component containing text.
        if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i32>>() {
            if utf8.len() == 1 {
                let string = utf8.value(0);
                ui.monospace(string);
                return;
            }
        }
        if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i64>>() {
            if utf8.len() == 1 {
                let string = utf8.value(0);
                ui.monospace(string);
                return;
            }
        }

        let num_bytes = array.total_size_bytes();
        if num_bytes < 3000 {
            if array.is_empty() {
                ui.monospace("[]");
                return;
            }

            let instance_count = array.len();
            let display = arrow2::array::get_display(array, "null");

            if instance_count == 1 {
                let mut string = String::new();
                match display(&mut string, 0) {
                    Ok(_) => ui.monospace(&string),
                    Err(err) => ui.error_with_details_on_hover(err.to_string()),
                };
                return;
            } else {
                ui.label(format!("{instance_count} items"))
                    .on_hover_ui(|ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        ui.monospace(format!(
                            "[{}]",
                            (0..instance_count)
                                .filter_map(|index| {
                                    let mut s = String::new();
                                    //TODO(ab): should we care about errors here?
                                    display(&mut s, index).ok().map(|_| s)
                                })
                                .join(", ")
                        ));
                    });
            }

            return;
        }

        // Fallback:
        let bytes = re_format::format_bytes(num_bytes as _);

        let data_type_formatted = format!("{:?}", array.data_type());

        if data_type_formatted.len() < 20 {
            // e.g. "4.2 KiB of Float32"
            ui.label(format!("{bytes} of {data_type_formatted}"));
        } else {
            // Huge datatype, probably a union horror show
            ui.label(format!("{bytes} of data"));
        }
    });
}
