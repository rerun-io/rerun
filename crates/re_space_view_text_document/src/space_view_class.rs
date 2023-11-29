use egui::Label;

use re_viewer_context::external::re_data_store::EntityProperties;
use re_viewer_context::{
    external::re_log_types::EntityPath, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewState, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartCollection,
    ViewQuery, ViewerContext,
};

use crate::view_part_system::TextDocumentEntry;

use super::view_part_system::TextDocumentSystem;

// TODO(andreas): This should be a blueprint component.

pub struct TextDocumentSpaceViewState {
    monospace: bool,
    word_wrap: bool,

    #[cfg(feature = "markdown")]
    commonmark_cache: egui_commonmark::CommonMarkCache,
}

impl Default for TextDocumentSpaceViewState {
    fn default() -> Self {
        Self {
            monospace: false,
            word_wrap: true,

            #[cfg(feature = "markdown")]
            commonmark_cache: Default::default(),
        }
    }
}

impl SpaceViewState for TextDocumentSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TextDocumentSpaceView;

impl SpaceViewClass for TextDocumentSpaceView {
    type State = TextDocumentSpaceViewState;

    const NAME: &'static str = "Text Document";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TEXTBOX
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "Displays text from a text entry components.".into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_part_system::<TextDocumentSystem>()
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
        ctx.re_ui.selection_grid(ui, "text_config").show(ui, |ui| {
            ctx.re_ui.grid_left_hand_label(ui, "Text style");
            ui.vertical(|ui| {
                ctx.re_ui
                    .radio_value(ui, &mut state.monospace, false, "Proportional");
                ctx.re_ui
                    .radio_value(ui, &mut state.monospace, true, "Monospace");
                ctx.re_ui.checkbox(ui, &mut state.word_wrap, "Word Wrap");
            });
            ui.end_row();
        });
    }

    fn ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        _view_ctx: &ViewContextCollection,
        parts: &ViewPartCollection,
        _query: &ViewQuery<'_>,
        _draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let text_document = parts.get::<TextDocumentSystem>()?;

        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if text_document.text_entries.is_empty() {
                            // We get here if we scroll back time to before the first text document was logged.
                            ui.weak("(empty)");
                        } else if text_document.text_entries.len() == 1 {
                            let TextDocumentEntry { body, media_type } =
                                &text_document.text_entries[0];

                            #[cfg(feature = "markdown")]
                            {
                                if media_type == &re_types::components::MediaType::markdown() {
                                    re_tracing::profile_scope!("egui_commonmark");

                                    // Make sure headers are big:
                                    ui.style_mut()
                                        .text_styles
                                        .entry(egui::TextStyle::Heading)
                                        .or_insert(egui::FontId::proportional(32.0))
                                        .size = 24.0;

                                    egui_commonmark::CommonMarkViewer::new("markdown_viewer")
                                        .max_image_width(Some(ui.available_width().floor() as _))
                                        .show(ui, &mut state.commonmark_cache, body);
                                    return;
                                }
                            }
                            #[cfg(not(feature = "markdown"))]
                            {
                                _ = media_type;
                            }

                            let mut text = egui::RichText::new(body.as_str());

                            if state.monospace {
                                text = text.monospace();
                            }

                            ui.add(Label::new(text).wrap(state.word_wrap));
                        } else {
                            // TODO(jleibs): better handling for multiple results
                            ui.label(format!(
                                "Can only show one text document at a time; was given {}.",
                                text_document.text_entries.len()
                            ));
                        }
                    })
            })
            .response
        });

        Ok(())
    }
}
