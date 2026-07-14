use re_chunk_store::UnitChunkShared;
use re_types_core::ComponentDescriptor;
use re_ui::{UiLayout, list_item};
use re_viewer_context::{AppContext, StoreViewContext};

use crate::{
    blob_ui::BlobUi, image_ui::ImageUi, transform_frames_ui::TransformFramesUi, video_ui::VideoUi,
};

pub enum ExtraDataUi {
    Video(VideoUi),
    Image(ImageUi),
    Blob(BlobUi),
    TransformHierarchy(TransformFramesUi),
}

impl ExtraDataUi {
    pub fn from_components(
        ctx: &StoreViewContext<'_>,
        entity_path: &re_log_types::EntityPath,
        descr: &ComponentDescriptor,
        chunk: &UnitChunkShared,
        entity_components: &[(ComponentDescriptor, UnitChunkShared)],
    ) -> Option<Self> {
        // Try video UI first.
        VideoUi::from_components(ctx, entity_path, descr)
            .map(Self::Video)
            .or_else(|| {
                BlobUi::from_components(ctx, entity_path, descr, chunk, entity_components)
                    .map(Self::Blob)
            })
            .or_else(|| {
                ImageUi::from_components(ctx, descr, chunk, entity_components).map(Self::Image)
            })
            .or_else(|| {
                TransformFramesUi::from_components(ctx, descr, chunk, entity_components)
                    .map(Self::TransformHierarchy)
            })
    }

    pub fn add_inline_buttons<'a>(
        &'a self,
        ctx: &'a AppContext<'_>,
        main_thread_token: re_capabilities::MainThreadToken,
        entity_path: &'a re_log_types::EntityPath,
        mut property_content: list_item::PropertyContent<'a>,
    ) -> list_item::PropertyContent<'a> {
        match self {
            Self::Video(_) => {
                // Video streams are not copyable or downloadable
                property_content
            }
            Self::Image(image) => {
                property_content = image.inline_copy_button(ctx, property_content);
                image.inline_download_button(ctx, main_thread_token, entity_path, property_content)
            }
            Self::Blob(blob) => blob.inline_download_button(ctx, entity_path, property_content),
            Self::TransformHierarchy(_) => {
                // Transform hierarchies are not copyable or dowloadable.
                property_content
            }
        }
    }

    pub fn data_ui(
        self,
        ctx: &StoreViewContext<'_>,
        ui: &mut egui::Ui,
        layout: UiLayout,
        entity_path: &re_log_types::EntityPath,
    ) {
        match self {
            Self::Video(video) => {
                video.data_ui(ctx, ui, layout);
            }
            Self::Image(image) => {
                image.data_ui(ctx, ui, layout, entity_path);
            }
            Self::Blob(blob) => {
                blob.data_ui(ctx, ui, layout, entity_path);
            }
            Self::TransformHierarchy(transform_hierarchy) => {
                transform_hierarchy.data_ui(ctx.app_ctx, ctx.db, ui, layout);
            }
        }
    }
}
