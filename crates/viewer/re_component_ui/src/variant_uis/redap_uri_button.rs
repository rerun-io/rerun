use std::error::Error;
use std::str::FromStr as _;

use re_types_core::{ComponentDescriptor, RowId};
use re_uri::RedapUri;
use re_viewer_context::ViewerContext;

/// Display an URL as an `Open` button (instead of spelling the full URL).
///
/// Requires a String mono-component which is valid [`RedapUri`].
pub fn redap_uri_button(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _component_descriptor: &ComponentDescriptor,
    _row_id: Option<RowId>,
    data: &dyn arrow::array::Array,
) -> Result<(), Box<dyn Error>> {
    if data.len() != 1 {
        return Err("component batches are not supported".into());
    }

    let url_str = data
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .ok_or_else(|| {
            format!(
                "unsupported arrow datatype: {}",
                re_arrow_util::format_data_type(data.data_type())
            )
        })?
        .value(0);

    let uri = RedapUri::from_str(url_str)?;

    //TODO(ab): we should provide feedback if the URI is already loaded, e.g. have "go to" instead of "open"
    if ui
        .button("Open")
        .on_hover_ui(|ui| {
            ui.label(uri.to_string());
        })
        .clicked()
    {
        let url = if ui.input(|i| i.modifiers.command) {
            egui::OpenUrl::new_tab(uri)
        } else {
            egui::OpenUrl::same_tab(uri)
        };

        ui.ctx().open_url(url);
    }

    Ok(())
}
