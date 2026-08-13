use std::error::Error;
use std::str::FromStr as _;

use egui::{Align2, AtomKind, Id, IntoAtoms as _, Ui};
use re_types_core::{ComponentIdentifier, RowId};
use re_ui::loading_indicator::paint_loading_indicator_inside;
use re_ui::{ReButton, Size, UiExt as _, Variant, icons};
use re_uri::RedapUri;
use re_viewer_context::open_url::ViewerOpenUrl;
use re_viewer_context::{AppContext, RecordingOrTable, SystemCommand, SystemCommandSender as _};

/// Display an URL as an `Open` button (instead of spelling the full URL).
///
/// Requires a String mono-component which is valid [`RedapUri`].
pub fn redap_uri_button(
    ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    _component: ComponentIdentifier,
    _row_id: Option<RowId>,
    array: &dyn arrow::array::Array,
) -> Result<(), Box<dyn Error>> {
    if array.len() != 1 {
        return Err("component batches are not supported".into());
    }

    let url_str = array
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .ok_or_else(|| format!("unsupported arrow datatype: {}", array.data_type()))?
        .value(0);

    let uri = RedapUri::from_str(url_str)?;

    let loaded_recording_info = ctx.store_bundle().recordings().find_map(|db| {
        if db
            .data_source
            .as_ref()
            .is_some_and(|source| source.stripped_redap_uri().as_ref() == Some(&uri))
        {
            db.store_info()
        } else {
            None
        }
    });
    let is_loading = loaded_recording_info.is_none()
        && ctx
            .connected_receivers
            .sources()
            .iter()
            .any(|source| source.stripped_redap_uri().as_ref() == Some(&uri));

    let uri_clone = uri.clone();

    let (open, loading, tooltip, opened_store_id) =
        if let Some(loaded_recording_info) = loaded_recording_info {
            let open = ctx.store_hub().is_opened(&loaded_recording_info.store_id);
            let opened_store_id = open.then(|| loaded_recording_info.store_id.clone());
            let tooltip = Some(if open {
                "This recording is already open. Click to switch to it."
            } else {
                "This recording is loaded. Click to open it."
            });
            (open, false, tooltip, opened_store_id)
        } else if is_loading {
            (true, true, None, None)
        } else {
            (false, false, None, None)
        };

    let mut atoms = match &uri {
        RedapUri::DatasetData(dataset)
            if ctx.active_redap_entry() == Some(dataset.dataset_id.into()) =>
        {
            // A segment is a recording within the active dataset; show just the segment button.
            re_viewer_context::segment_button_atoms(dataset.segment_id.as_str(), ui.ctx().theme())
        }
        _ => re_ui::UrlDecorator::get(ui.ctx())
            .and_then(|decorator| decorator(url_str))
            .map(|link| link.into_atoms())
            .unwrap_or_else(|| url_str.into_atoms()),
    };

    let spinner_id = Id::new("loading_spinner");

    if loading {
        let mut mapped_icon = false;
        atoms.map_atoms(|mut atom| {
            if !mapped_icon && matches!(atom.kind, AtomKind::Image(..)) {
                atom.kind = AtomKind::Empty;
                atom.size = Some(ui.tokens().small_icon_size);
                atom.id = Some(spinner_id);
                mapped_icon = true;
            }
            atom
        });
    }

    let size = Size::Tiny;
    let default_variant = Variant::Ghost;

    let button = || {
        ReButton::new(atoms.clone())
            .size(size)
            .variant(if open {
                Variant::Opened
            } else {
                default_variant
            })
            // Some icons have blue arrows, so we can't tint them.
            .image_tint_follows_text_color(false)
    };

    let count = if open { 2.0 } else { 1.0 };
    let icon_button_width = count * size.height() + ui.spacing().item_spacing.x * (count - 1.0);

    let (mut response, icon_responses) =
        ReButton::with_hover_icon_buttons(ui, button, icon_button_width, |ui| {
            (
                ui.add(
                    ReButton::icon(icons::COPY)
                        .size(size)
                        .variant(default_variant),
                )
                .clicked(),
                if open {
                    ui.add(
                        ReButton::icon(icons::CLOSE_SMALL)
                            .size(size)
                            .variant(default_variant),
                    )
                    .on_hover_text(if loading { "Cancel" } else { "Close" })
                    .clicked()
                } else {
                    false
                },
            )
        });

    if let Some(tooltip) = tooltip {
        response.response = response.response.on_hover_text(tooltip);
    }

    if let Some(rect) = response.rect(spinner_id) {
        paint_loading_indicator_inside(
            ui,
            Align2::CENTER_CENTER,
            rect,
            1.0,
            None,
            "Loading recording",
        );
    }

    handle_open_full_recording_link(ui, uri.clone(), &response);

    if let Some((copy_clicked, close_clicked)) = icon_responses {
        if copy_clicked {
            if let Ok(url) = ViewerOpenUrl::from(uri_clone.clone()).sharable_url(None) {
                ctx.command_sender
                    .send_system(SystemCommand::CopyViewerUrl(url));
            } else {
                re_log::error!("Failed to create a sharable url for recording");
            }
        }
        if close_clicked {
            if let Some(store_id) = opened_store_id {
                // The recording is already loaded — close it and free its memory.
                ctx.command_sender
                    .send_system(SystemCommand::CloseRecordingOrTable(
                        RecordingOrTable::Recording { store_id },
                    ));
            } else {
                // Still loading — cancel the connected receiver.
                ctx.connected_receivers.remove_by_uri(&uri.to_string());
            }
        }
    }

    Ok(())
}

fn handle_open_full_recording_link(ui: &Ui, uri: RedapUri, response: &egui::Response) {
    if response.clicked_with_open_in_background() {
        ui.open_url(egui::OpenUrl::new_tab(uri));
    } else if response.clicked() {
        ui.open_url(egui::OpenUrl::same_tab(uri));
    }
}
