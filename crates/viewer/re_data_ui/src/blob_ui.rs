use std::sync::Arc;

use re_chunk_store::UnitChunkShared;
use re_log_types::EntityPath;
use re_sdk_types::components::{Blob, MediaType, VideoTimestamp};
use re_sdk_types::{ComponentDescriptor, ComponentIdentifier, RowId, archetypes, components};
use re_types_core::Component as _;
use re_ui::list_item::{self, ListItemContentButtonsExt as _, PropertyContent};
use re_ui::{UiExt as _, icons};
use re_viewer_context::{StoredBlobCacheKey, UiLayout, ViewerContext};

use crate::image_ui::ImageUi;
use crate::video_ui::VideoUi;
use crate::{EntityDataUi, find_and_deserialize_archetype_mono_component};

impl EntityDataUi for Blob {
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        component_descriptor: &ComponentDescriptor,
        row_id: Option<RowId>,
        query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let compact_size_string = re_format::format_bytes(self.len() as _);

        // We show the actual mime of the blob here instead of doing
        // a side-lookup of the sibling `MediaType` component.
        // This is part of "showing the data as it is".
        // If the user clicked on the blob, is because they want to see info about the blob,
        // not about a sibling component.
        // This can also help a user debug if they log the contents of `.png` file with a `image/jpeg` `MediaType`.
        let media_type = MediaType::guess_from_data(self);

        let blob_ui = BlobUi::new(
            ctx,
            entity_path,
            component_descriptor,
            row_id,
            self.0.clone(),
            media_type.as_ref(),
            None,
        );

        if ui_layout.is_single_line() {
            ui.horizontal(|ui| {
                ui.set_truncate_style();
                blob_ui.data_ui(ctx, ui, ui_layout, query, entity_path);

                ui.label(compact_size_string);

                if let Some(media_type) = &media_type {
                    ui.label(media_type.to_string())
                        .on_hover_text("Media type (MIME) based on magic header bytes");
                }
            });
        } else {
            let all_digits_size_string = format!("{} B", re_format::format_uint(self.len()));
            let size_string = if self.len() < 1024 {
                all_digits_size_string
            } else {
                format!("{all_digits_size_string} ({compact_size_string})")
            };

            re_ui::list_item::list_item_scope(ui, "blob_info", |ui| {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Size").value_text(size_string),
                );

                if let Some(media_type) = &media_type {
                    ui.list_item_flat_noninteractive(
                        PropertyContent::new("Media type").value_text(media_type.as_str()),
                    )
                    .on_hover_text("Media type (MIME) based on magic header bytes");
                } else {
                    ui.list_item_flat_noninteractive(
                        PropertyContent::new("Media type").value_text("?"),
                    )
                    .on_hover_text("Failed to detect media type (Mime) from magic header bytes");
                }
                blob_ui.data_ui(ctx, ui, ui_layout, query, entity_path);
            });
        }
    }
}

/// Show EXIF data about the given blob (image), if possible.
fn exif_ui(ui: &mut egui::Ui, key: StoredBlobCacheKey, blob: &re_sdk_types::datatypes::Blob) {
    let exif_result = ui.ctx().memory_mut(|mem| {
        // Cache EXIF parsing to avoid re-parsing every frame.
        // The parsing is really fast, so this is not really needed.
        let cache = mem
            .caches
            .cache::<egui::cache::FramePublisher<StoredBlobCacheKey, Arc<rexif::ExifResult>>>();
        cache.get(&key).cloned().unwrap_or_else(|| {
            re_tracing::profile_scope!("exif-parse");
            let (result, _warnings) = rexif::parse_buffer_quiet(blob);
            let result = Arc::new(result);
            cache.set(key, result.clone());
            result
        })
    });

    if let Ok(exif) = &*exif_result {
        ui.list_item_collapsible_noninteractive_label("EXIF", false, |ui| {
            list_item::list_item_scope(ui, "exif", |ui| {
                for entry in &exif.entries {
                    let tag_string = if entry.tag == rexif::ExifTag::UnknownToMe {
                        "<Unknown tag>".to_owned()
                    } else {
                        entry.tag.to_string()
                    };
                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new(tag_string)
                            .value_text(entry.value_more_readable.to_string()),
                    );
                }
            });
        });
    }
}

/// Utility for displaying additional UI for blobs.
pub struct BlobUi {
    component: ComponentIdentifier,
    blob: re_sdk_types::datatypes::Blob,

    /// Additional image ui if any.
    image: Option<ImageUi>,

    /// Additional video ui if the blob is a video.
    video: Option<VideoUi>,

    /// The row id of the blob.
    row_id: Option<RowId>,

    /// The media type of the blob if known (used to inform image and video uis).
    media_type: Option<MediaType>,
}

impl BlobUi {
    pub fn from_components(
        ctx: &ViewerContext<'_>,
        entity_path: &re_log_types::EntityPath,
        blob_descr: &ComponentDescriptor,
        blob_chunk: &UnitChunkShared,
        components: &[(ComponentDescriptor, UnitChunkShared)],
    ) -> Option<Self> {
        if blob_descr.component_type != Some(components::Blob::name()) {
            return None;
        }

        let blob = blob_chunk
            .component_mono::<components::Blob>(blob_descr.component)?
            .ok()?;

        // Media type comes typically alongside the blob in various different archetypes.
        // Look for the one that matches the blob's archetype.
        let media_type = find_and_deserialize_archetype_mono_component::<components::MediaType>(
            components,
            blob_descr.archetype,
        )
        .or_else(|| components::MediaType::guess_from_data(&blob));

        // Video timestamp is only relevant here if it comes from a VideoFrameReference archetype.
        // It doesn't show up in the blob's archetype.
        let video_timestamp_descr = archetypes::VideoFrameReference::descriptor_timestamp();
        let video_timestamp = components
            .iter()
            .find_map(|(descr, chunk)| {
                (descr == &video_timestamp_descr).then(|| {
                    chunk
                        .component_mono::<components::VideoTimestamp>(
                            video_timestamp_descr.component,
                        )?
                        .ok()
                })
            })
            .flatten();

        Some(Self::new(
            ctx,
            entity_path,
            blob_descr,
            blob_chunk.row_id(),
            blob.0,
            media_type.as_ref(),
            video_timestamp,
        ))
    }

    pub fn new(
        ctx: &re_viewer_context::ViewerContext<'_>,
        entity_path: &re_log_types::EntityPath,
        blob_component_descriptor: &ComponentDescriptor,
        blob_row_id: Option<RowId>,
        blob: re_sdk_types::datatypes::Blob,
        media_type: Option<&MediaType>,
        video_timestamp: Option<VideoTimestamp>,
    ) -> Self {
        let (image, video) = if let Some(blob_row_id) = blob_row_id {
            (
                ImageUi::from_blob(
                    ctx,
                    blob_row_id,
                    blob_component_descriptor,
                    &blob,
                    media_type,
                ),
                VideoUi::from_blob(
                    ctx,
                    entity_path,
                    blob_row_id,
                    blob_component_descriptor,
                    &blob,
                    media_type,
                    video_timestamp,
                ),
            )
        } else {
            (None, None)
        };

        Self {
            image,
            video,
            row_id: blob_row_id,
            component: blob_component_descriptor.component,
            blob,
            media_type: media_type.cloned(),
        }
    }

    pub fn inline_download_button<'a>(
        &'a self,
        ctx: &'a ViewerContext<'_>,
        entity_path: &'a EntityPath,
        mut property_content: list_item::PropertyContent<'a>,
    ) -> list_item::PropertyContent<'a> {
        if let Some(image) = &self.image {
            property_content = image.inline_copy_button(ctx, property_content);
        }
        property_content.with_action_button(&icons::DOWNLOAD, "Save blob…", || {
            let mut file_name = entity_path
                .last()
                .map_or("blob", |name| name.unescaped_str())
                .to_owned();

            if let Some(file_extension) =
                self.media_type.as_ref().and_then(|mt| mt.file_extension())
            {
                file_name.push('.');
                file_name.push_str(file_extension);
            }

            ctx.command_sender().save_file_dialog(
                re_capabilities::MainThreadToken::i_promise_i_am_on_the_main_thread(),
                &file_name,
                "Save blob".to_owned(),
                self.blob.to_vec(),
            );
        })
    }

    pub fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &EntityPath,
    ) {
        if let Some(row_id) = self.row_id
            && ui_layout == UiLayout::SelectionPanel
        {
            exif_ui(
                ui,
                StoredBlobCacheKey::new(row_id, self.component),
                &self.blob,
            );
        }

        if let Some(image) = &self.image {
            image.data_ui(ctx, ui, ui_layout, query, entity_path);
        }

        if let Some(video) = &self.video {
            video.data_ui(ctx, ui, ui_layout, query);
        }
    }
}
