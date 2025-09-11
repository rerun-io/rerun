use crate::image::ImageUi;
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

pub enum ExtraDataUi {
    Video {},
    Image(image::ImageUi),
    Blob(blob::BlobUi),
}

impl ExtraDataUi {
    pub fn from_components(
        ctx: &ViewerContext<'_>,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &re_log_types::EntityPath,
        descr: &ComponentDescriptor,
        chunk: &UnitChunkShared,
        entity_components: &[(ComponentDescriptor, UnitChunkShared)],
    ) -> Option<Self> {
        let image =
            ImageUi::from_components(ctx, descr, chunk, entity_components).map(ExtraDataUi::Image);
        let blob =
            blob::BlobUi::from_components(ctx, query, entity_path, descr, chunk, entity_components)
                .map(ExtraDataUi::Blob);
        image.or(blob)
    }

    pub fn add_inline_buttons<'a>(
        &'a self,
        ctx: &'a ViewerContext<'_>,
        entity_path: &'a re_log_types::EntityPath,
        mut property_content: list_item::PropertyContent<'a>,
    ) -> list_item::PropertyContent<'a> {
        match self {
            ExtraDataUi::Video { .. } => property_content,
            ExtraDataUi::Image(image) => {
                property_content = image.inline_copy_button(ctx, property_content);
                image.inline_download_button(ctx, entity_path, property_content)
            }
            ExtraDataUi::Blob(blob) => {
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
            ExtraDataUi::Video { .. } => {}
            ExtraDataUi::Image(image) => {
                image.data_ui(ctx, ui, layout, query, entity_path);
            }
            ExtraDataUi::Blob(blob) => {
                blob.data_ui(ctx, ui, layout, query, entity_path);
            }
        }
    }
}
