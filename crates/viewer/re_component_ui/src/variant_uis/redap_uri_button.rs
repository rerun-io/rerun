use std::error::Error;
use std::str::FromStr as _;

use re_types_core::{ComponentDescriptor, RowId};
use re_uri::RedapUri;
use re_viewer_context::{SystemCommand, SystemCommandSender, ViewerContext};

/// Display an URL as an `Open` button (instead of spelling the full URL).
///
/// Requires a String mono-component which is valid [`RedapUri`].
pub fn redap_uri_button(
    ctx: &ViewerContext<'_>,
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

    let loaded_recording_id = ctx.storage_context.bundle.entity_dbs().find_map(|db| {
        if db
            .data_source
            .as_ref()
            .is_some_and(|source| source.redap_uri().as_ref() == Some(&uri))
        {
            Some(db.store_id())
        } else {
            None
        }
    });

    if let Some(loaded_recording_id) = loaded_recording_id {
        let response = ui.button("Switch to").on_hover_ui(|ui| {
            ui.label("This recording is already loaded. Click to switch to it.");
        });

        if response.clicked() {
            // Show it:
            ctx.command_sender()
                .send_system(SystemCommand::ChangeDisplayMode(
                    re_viewer_context::DisplayMode::RedapServer(uri.origin().clone()),
                ));
            ctx.command_sender()
                .send_system(SystemCommand::SetSelection(
                    re_viewer_context::Item::StoreId(loaded_recording_id),
                ));
        }
    } else {
        let response = ui.button("Open").on_hover_ui(|ui| {
            ui.label(uri.to_string());
        });

        if response.middle_clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(uri));
        } else if response.clicked() {
            let url = if ui.input(|i| i.modifiers.any()) {
                egui::OpenUrl::new_tab(uri)
            } else {
                egui::OpenUrl::same_tab(uri)
            };

            ui.ctx().open_url(url);
        }
    }

    Ok(())
}
