use std::error::Error;

use re_protos::cloud::v1alpha1::EntryKind;
use re_types_core::{ComponentIdentifier, RowId};
use re_viewer_context::ViewerContext;

/// Parse an `Int32Array` as an `EntryKind` and display it.
pub fn redap_entry_kind(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _component: ComponentIdentifier,
    _row_id: Option<RowId>,
    array: &dyn arrow::array::Array,
) -> Result<(), Box<dyn Error>> {
    if array.len() != 1 {
        return Err("component batches are not supported".into());
    }

    let value = array
        .as_any()
        .downcast_ref::<arrow::array::Int32Array>()
        .ok_or_else(|| {
            format!(
                "unsupported arrow datatype: {}",
                re_arrow_util::format_data_type(array.data_type())
            )
        })?
        .value(0);

    let kind = EntryKind::try_from(value);
    let name = kind
        .as_ref()
        .map_or("Unknown EntryKind", EntryKind::display_name);
    ui.label(name);

    Ok(())
}
