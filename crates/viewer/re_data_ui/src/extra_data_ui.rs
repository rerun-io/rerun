use re_chunk_store::UnitChunkShared;
use re_types_core::ComponentDescriptor;
use re_ui::{UiLayout, list_item};
use re_viewer_context::ViewerContext;

use crate::{blob, image, transform_frames, video};

pub enum ExtraDataUi {
    Video(video::VideoUi),
    Image(image::ImageUi),
    Blob(blob::BlobUi),
    TransformHierarchy(transform_frames::TransformFramesUi),
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
        blob::BlobUi::from_components(ctx, entity_path, descr, chunk, entity_components)
            .map(Self::Blob)
            .or_else(|| {
                image::ImageUi::from_components(ctx, descr, chunk, entity_components)
                    .map(Self::Image)
            })
            .or_else(|| {
                video::VideoUi::from_components(ctx, query, entity_path, descr).map(Self::Video)
            })
            .or_else(|| {
                transform_frames::TransformFramesUi::from_components(
                    ctx,
                    query,
                    descr,
                    chunk,
                    entity_components,
                )
                .map(Self::TransformHierarchy)
            })
    }

    pub fn add_inline_buttons<'a>(
        &'a self,
        ctx: &'a ViewerContext<'_>,
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
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &re_log_types::EntityPath,
    ) {
        match self {
            Self::Video(video) => {
                video.data_ui(ctx, ui, layout, query);
            }
            Self::Image(image) => {
                image.data_ui(ctx, ui, layout, query, entity_path);
            }
            Self::Blob(blob) => {
                blob.data_ui(ctx, ui, layout, query, entity_path);
            }
            Self::TransformHierarchy(transform_hierarchy) => {
                transform_hierarchy.data_ui(ctx, ui, layout);
            }
        }
    }
}
