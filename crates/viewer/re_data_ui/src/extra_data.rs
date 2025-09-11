use crate::image::ImageExtraData;
use crate::{blob, image};
use egui::Rangef;
use re_chunk_store::UnitChunkShared;
use re_types::components;
use re_types::components::ValueRange;
use re_types::image::ImageKind;
use re_types_core::{Component, ComponentDescriptor, ComponentType};
use re_ui::{UiLayout, icons, list_item};
use re_viewer_context::gpu_bridge::image_data_range_heuristic;
use re_viewer_context::{ColormapWithRange, ImageInfo, ImageStats, ImageStatsCache, ViewerContext};

pub enum ExtraData {
    Video {},
    Image(image::ImageExtraData),
    Blob(blob::BlobExtraData),
}

impl ExtraData {
    pub fn get(
        ctx: &ViewerContext<'_>,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &re_log_types::EntityPath,
        descr: &ComponentDescriptor,
        chunk: &UnitChunkShared,
        entity_components: &[(ComponentDescriptor, UnitChunkShared)],
    ) -> Option<Self> {
        let image = ImageExtraData::get(ctx, descr, chunk, entity_components).map(ExtraData::Image);
        let blob = blob::BlobExtraData::get_from_components(
            ctx,
            query,
            entity_path,
            descr,
            chunk,
            entity_components,
        )
        .map(ExtraData::Blob);
        image.or(blob)
    }

    pub fn add_inline_buttons<'a>(
        &'a self,
        ctx: &'a ViewerContext<'_>,
        entity_path: &'a re_log_types::EntityPath,
        mut property_content: list_item::PropertyContent<'a>,
    ) -> list_item::PropertyContent<'a> {
        match self {
            ExtraData::Video { .. } => property_content,
            ExtraData::Image(image) => {
                property_content = image.inline_copy_button(ctx, property_content);
                image.inline_download_button(ctx, entity_path, property_content)
            }
            ExtraData::Blob(blob) => {
                blob.inline_download_button(ctx, entity_path, property_content)
            }
        }
    }

    pub fn data_ui(
        self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &re_log_types::EntityPath,
    ) {
        match self {
            ExtraData::Video { .. } => {}
            ExtraData::Image(image) => {
                image.data_ui(ctx, ui, layout, query, entity_path);
            }
            ExtraData::Blob(blob) => {
                blob.data_ui(ctx, ui, layout, query, entity_path);
            }
        }
    }
}
