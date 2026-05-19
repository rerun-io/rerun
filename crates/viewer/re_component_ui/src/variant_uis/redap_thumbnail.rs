use std::error::Error;

use re_sdk_types::components::MediaType;
use re_types_core::{ComponentIdentifier, Loggable as _, RowId};
use re_ui::UiLayout;
use re_viewer_context::StoreViewContext;

/// Display a thumbnail that takes all the available space.
pub fn redap_thumbnail(
    ctx: &StoreViewContext<'_>,
    ui: &mut egui::Ui,
    component: ComponentIdentifier,
    row_id: Option<RowId>,
    data: &dyn arrow::array::Array,
) -> Result<(), Box<dyn Error>> {
    let row_id = row_id.ok_or("RowId is required for redap_thumbnail")?;

    let blobs = re_sdk_types::components::Blob::from_arrow(data)?;
    let blob = blobs.first().ok_or("Blob data is empty")?;

    let slice = blob.as_ref();

    let media_type = MediaType::guess_from_data(slice);

    let image = ctx.memoizer(|c: &mut re_viewer_context::ImageDecodeCache| {
        #[expect(deprecated)] // TODO(isse): Figure out a way to do this using the video decoder.
        c.entry_encoded_color(row_id, component, slice, media_type.as_ref())
    })?;

    re_data_ui::image_preview_ui(
        ctx,
        None, // Can't look up annotations for segmentation images
        ui,
        UiLayout::List,
        &re_log_types::EntityPath::from("redap_thumbnail"),
        &image,
        None,
    );

    Ok(())
}
