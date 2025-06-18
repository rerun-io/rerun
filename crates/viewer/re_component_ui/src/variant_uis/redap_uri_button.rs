use std::error::Error;
use std::str::FromStr as _;

use egui::{Align, Layout, Link, Ui, UiBuilder};
use re_types_core::{ComponentDescriptor, RowId};
use re_uri::RedapUri;
use re_viewer_context::{SystemCommand, SystemCommandSender as _, ViewerContext};

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

    let put_left_aligned = |ui: &mut Ui, link| {
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(ui.max_rect())
                .layout(Layout::top_down_justified(Align::Min)),
            |ui| ui.add(link),
        )
        .inner
    };

    if let Some(loaded_recording_id) = loaded_recording_id {
        let response = put_left_aligned(ui, Link::new("Switch to")).on_hover_ui(|ui| {
            ui.label("This recording is already loaded. Click to switch to it.");
        });

        if response.clicked() {
            // Show it:
            ctx.command_sender()
                .send_system(SystemCommand::SetSelection(
                    re_viewer_context::Item::StoreId(loaded_recording_id),
                ));
        }
    } else {
        let response = put_left_aligned(ui, Link::new("Open")).on_hover_ui(|ui| {
            ui.label(uri.to_string());
        });

        if response.clicked_with_open_in_background() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(uri));
        } else if response.clicked() {
            ui.ctx().open_url(egui::OpenUrl::same_tab(uri));
        }
    }

    Ok(())
}
