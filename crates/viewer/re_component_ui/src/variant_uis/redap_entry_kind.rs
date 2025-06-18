use re_protos::catalog::v1alpha1::EntryKind;
use re_types_core::{ComponentDescriptor, RowId};
use re_viewer_context::ViewerContext;
use std::error::Error;

/// Parse an Int32Array as an EntryKind and display it.
pub fn redap_entry_kind(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _component_descriptor: &ComponentDescriptor,
    _row_id: Option<RowId>,
    data: &dyn arrow::array::Array,
) -> Result<(), Box<dyn Error>> {
    if data.len() != 1 {
        return Err("component batches are not supported".into());
    }

    let value = data
        .as_any()
        .downcast_ref::<arrow::array::Int32Array>()
        .ok_or_else(|| {
            format!(
                "unsupported arrow datatype: {}",
                re_arrow_util::format_data_type(data.data_type())
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
