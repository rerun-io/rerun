use arrow::array::{Array as _, ListArray, UInt8Array};
use re_log_types::TimelineName;
use re_types::components::MediaType;
use re_types_core::{ComponentDescriptor, RowId};
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

    let blob = data.as_any().downcast_ref::<ListArray>().ok_or_else(|| {
        format!(
            "unsupported arrow datatype: {}",
            re_arrow_util::format_data_type(data.data_type())
        )
    })?;

    let values = blob
        .values()
        .as_any()
        .downcast_ref::<UInt8Array>()
        .ok_or_else(|| {
            format!(
                "unsupported arrow datatype for values: {}",
                re_arrow_util::format_data_type(blob.values().data_type())
            )
        })?;

    let slice: &[u8] = &values.values()[..];

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
