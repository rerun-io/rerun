use re_log_types::TimelineName;
use re_types::components::MediaType;
use re_types_core::{ComponentDescriptor, Loggable as _, RowId};
use re_ui::UiLayout;
use re_viewer_context::ViewerContext;
use re_viewer_context::external::re_chunk_store::LatestAtQuery;
use std::error::Error;

/// Display a thumbnail that takes all the available space.
pub fn redap_thumbnail(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    component_descriptor: &ComponentDescriptor,
    row_id: Option<RowId>,
    data: &dyn arrow::array::Array,
) -> Result<(), Box<dyn Error>> {
    let row_id = row_id.ok_or("RowId is required for redap_thumbnail")?;

    let blobs = re_types::components::Blob::from_arrow(data)?;
    let blob = blobs.first().ok_or("Blob data is empty")?;

    let slice = blob.as_ref();

    let media_type = MediaType::guess_from_data(slice);

    let image = ctx
        .store_context
        .caches
        .entry(|c: &mut re_viewer_context::ImageDecodeCache| {
            c.entry(row_id, component_descriptor, slice, media_type.as_ref())
        })?;

    re_data_ui::image_preview_ui(
        ctx,
        ui,
        UiLayout::List,
        &LatestAtQuery::latest(TimelineName::new("unknown")),
        &re_log_types::EntityPath::from("redap_thumbnail"),
        &image,
        None,
    );

    Ok(())
}
