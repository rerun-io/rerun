use std::error::Error;
use std::str::FromStr as _;

use egui::{Align, Layout, Link, Ui, UiBuilder};
use re_types_core::{ComponentDescriptor, RowId};
use re_ui::UiExt as _;
use re_uri::RedapUri;
use re_viewer_context::{
    SystemCommand, SystemCommandSender as _, ViewerContext, open_url::ViewerOpenUrl,
};

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

    let loaded_recording_id = ctx.storage_context.bundle.recordings().find_map(|db| {
        if db
            .data_source
            .as_ref()
            .is_some_and(|source| source.redap_uri().as_ref() == Some(&uri))
        {
            Some(db.store_id().clone())
        } else {
            None
        }
    });
    let is_loading = loaded_recording_id.is_none()
        && ctx
            .connected_receivers
            .sources()
            .iter()
            .any(|source| source.redap_uri().as_ref() == Some(&uri));

    let uri_clone = uri.clone();
    // Show the link left aligned and justified so the whole cell is clickable.
    //
    // And add a button to copy the link.
    let link_with_copy = |ui: &mut Ui, link| {
        let rect = ui.max_rect();
        let contains_pointer = ui.rect_contains_pointer(rect);
        egui::Sides::new()
            .shrink_left()
            .show(
                ui,
                |ui| {
                    ui.scope_builder(
                        UiBuilder::new()
                            .max_rect(egui::Rect::from_x_y_ranges(
                                ui.max_rect().x_range(),
                                rect.y_range(),
                            ))
                            .layout(
                                Layout::left_to_right(Align::Center)
                                    .with_main_justify(true)
                                    .with_cross_justify(true)
                                    .with_main_align(Align::Min),
                            ),
                        |ui| ui.add(link),
                    )
                    .inner
                },
                |ui| {
                    if contains_pointer
                        && ui
                            .small_icon_button(&re_ui::icons::COPY, "Copy link")
                            .clicked()
                    {
                        if let Ok(url) = ViewerOpenUrl::from(uri_clone).sharable_url(None) {
                            ctx.command_sender()
                                .send_system(SystemCommand::CopyViewerUrl(url));
                        } else {
                            re_log::error!("Failed to create a sharable url for recording");
                        }
                    }
                },
            )
            .0
    };

    if let Some(loaded_recording_id) = loaded_recording_id {
        let response = link_with_copy(ui, Link::new("Switch to")).on_hover_ui(|ui| {
            ui.label("This recording is already loaded. Click to switch to it.");
        });

        if response.clicked() {
            // Show it:
            ctx.command_sender()
                .send_system(SystemCommand::SetSelection(
                    re_viewer_context::Item::StoreId(loaded_recording_id).into(),
                ));
        }
    } else if is_loading {
        ui.horizontal(|ui| {
            ui.spinner();

            if ui
                .small_icon_button(&re_ui::icons::CLOSE_SMALL, "Cancel loading the recording")
                .on_hover_text("Cancel")
                .clicked()
            {
                ctx.connected_receivers.remove_by_uri(&uri.to_string());
            }
        });
    } else {
        let response = link_with_copy(ui, Link::new("Open")).on_hover_ui(|ui| {
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
