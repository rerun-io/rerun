use re_chunk_store::UnitChunkShared;
use re_log_types::EntityPath;
use re_types::{
    ComponentDescriptor, TransformFrameIdHash, archetypes,
    components::{self, TransformFrameId},
};
use re_ui::{HasDesignTokens as _, UiExt as _, UiLayout, icons};
use re_viewer_context::{
    Item, SystemCommand, SystemCommandSender as _, TransformDatabaseStoreCache, ViewerContext,
};

/// The max amount of ancestors we show before putting a '…'.
const MAX_SHOWN_ANCESTORS: usize = 10;

/// The max amount of ancestors we show in a tooltip.
///
/// We want this to be less because we don't use a scroll area for
/// tooltips.
const MAX_SHOWN_ANCESTORS_TOOLTIP: usize = 3;

struct TransformFrameInfo {
    frame_id: TransformFrameId,
    source_entity: Option<EntityPath>,
}

pub struct TransformFramesUi {
    frames: Vec<TransformFrameInfo>,

    /// True if there are more transform frames than the ones shown here.
    more: bool,
}

impl TransformFramesUi {
    pub fn from_components(
        ctx: &ViewerContext<'_>,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &EntityPath,
        transform_frame_descr: &ComponentDescriptor,
        transform_frame_chunk: &UnitChunkShared,
        entity_components: &[(ComponentDescriptor, UnitChunkShared)],
    ) -> Option<Self> {
        // All frame descriptor components, sorted by priority.
        //
        // The bool indicates if we should use the entity path as
        // the current frame.
        let frame_components = [
            (
                archetypes::Transform3D::descriptor_child_frame().component,
                false,
            ),
            (
                archetypes::Pinhole::descriptor_child_frame().component,
                false,
            ),
            (
                archetypes::CoordinateFrame::descriptor_frame().component,
                false,
            ),
            (
                archetypes::Pinhole::descriptor_parent_frame().component,
                true,
            ),
            (
                archetypes::Transform3D::descriptor_parent_frame().component,
                true,
            ),
        ];

        let find_frame_component = |other_component| {
            frame_components
                .iter()
                .enumerate()
                .map(|(priority, (component, use_entity_path))| {
                    (priority, *component, *use_entity_path)
                })
                .find(|(_, component, _)| *component == other_component)
        };

        let (priority, component, use_entity_path) =
            find_frame_component(transform_frame_descr.component)?;

        let selected_view_id = ctx
            .selection()
            .single_item()
            .and_then(|item| item.view_id());

        if entity_components
            .iter()
            .filter(|(desc, _)| desc.component != component)
            .filter_map(|(desc, _)| find_frame_component(desc.component))
            .any(|(p, ..)| p < priority)
        {
            return None;
        }
        let mut frame_id_hash = if use_entity_path {
            TransformFrameIdHash::from_entity_path(entity_path)
        } else {
            let frame_id = transform_frame_chunk
                .component_mono::<components::TransformFrameId>(transform_frame_descr.component)?
                .ok()?;

            TransformFrameIdHash::new(&frame_id)
        };

        let caches = ctx.store_context.caches;
        let transform_cache = caches
            .entry(|c: &mut TransformDatabaseStoreCache| c.lock_transform_cache(ctx.recording()));

        let frame_ids = transform_cache.frame_id_registry();
        let transforms = transform_cache.transforms_for_timeline(*ctx.time_ctrl.timeline().name());

        let mut frames = Vec::new();

        let mut i = 0;

        // Collect transform frame ancestors.
        let more = loop {
            let Some(frame_id) = frame_ids.lookup_frame_id(frame_id_hash) else {
                break false;
            };

            let Some(frame) = transforms.frame_transforms(frame_id_hash) else {
                frames.push(TransformFrameInfo {
                    frame_id: frame_id.clone(),
                    source_entity: None,
                });

                break false;
            };

            frames.push(TransformFrameInfo {
                frame_id: frame_id.clone(),
                source_entity: Some(frame.associated_entity_path().clone()),
            });

            let Some(transform) = frame.latest_at_transform(ctx.recording(), query) else {
                break false;
            };

            frame_id_hash = transform.parent;

            i += 1;

            if i >= MAX_SHOWN_ANCESTORS {
                break true;
            }
        };

        Some(Self { frames, more })
    }

    pub fn data_ui(&self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui, layout: UiLayout) {
        egui::Frame::new()
            .corner_radius(ui.visuals().menu_corner_radius)
            .fill(ui.visuals().tokens().text_edit_bg_color)
            .inner_margin(8.0)
            .show(ui, |ui| match layout {
                UiLayout::Tooltip => self.show_transforms(ctx, layout, ui),
                UiLayout::List | UiLayout::SelectionPanel => {
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            self.show_transforms(ctx, layout, ui);
                        });
                }
            });
    }

    fn show_transforms(&self, ctx: &ViewerContext<'_>, layout: UiLayout, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            let show_amount = match layout {
                UiLayout::Tooltip => MAX_SHOWN_ANCESTORS_TOOLTIP,
                UiLayout::SelectionPanel | UiLayout::List => MAX_SHOWN_ANCESTORS,
            };
            let more = self.more || self.frames.len() > show_amount;

            if more {
                ui.add(egui::Label::new("…").selectable(false))
                    .on_hover_text("There are more frames not displayed here");
            }

            for (idx, transform) in self.frames.iter().take(show_amount).enumerate().rev() {
                if idx + 1 < self.frames.len() || more {
                    let id = ui.next_auto_id();
                    let rect = ui.small_icon(&icons::ARROW_UP, None);
                    ui.interact(rect, id, egui::Sense::hover())
                        .on_hover_text(format!(
                            "{} is a child frame of {}",
                            transform.frame_id,
                            self.frames
                                .get(idx + 1)
                                .map(|transform| transform.frame_id.as_str())
                                .unwrap_or("another frame")
                        ));
                }

                let is_current = idx == 0;

                transform_ui(ctx, ui, transform, is_current, layout);
            }
        });
    }
}

fn transform_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    transform: &TransformFrameInfo,
    is_current: bool,
    layout: UiLayout,
) {
    match layout {
        UiLayout::Tooltip => {
            ui.label(transform.frame_id.as_str());
        }
        UiLayout::List | UiLayout::SelectionPanel => {
            let response = ui
                .add_enabled(
                    transform.source_entity.is_some(),
                    egui::Button::selectable(is_current, transform.frame_id.as_str()),
                )
                .on_disabled_hover_text("No related entity found for frame");

            if let Some(source_entity) = &transform.source_entity
                && response.on_hover_text(source_entity.to_string()).clicked()
            {
                let selected_view_id = ctx
                    .selection()
                    .single_item()
                    .and_then(|item| item.view_id());

                let item = if let Some(selected_view) = selected_view_id {
                    Item::DataResult(selected_view, source_entity.clone().into())
                } else {
                    Item::from(source_entity.clone())
                };

                ctx.command_sender()
                    .send_system(SystemCommand::set_selection(item));
            }
        }
    }
}
