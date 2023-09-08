use egui::Label;
use re_viewer_context::{
    external::re_log_types::EntityPath, SpaceViewClass, SpaceViewClassName,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartCollection, ViewQuery, ViewerContext,
};

use super::view_part_system::TextDocumentSystem;

// TODO(andreas): This should be a blueprint component.
#[derive(Clone, PartialEq, Eq)]
pub struct TextDocumentSpaceViewState {
    monospace: bool,
    word_wrap: bool,
}

impl Default for TextDocumentSpaceViewState {
    fn default() -> Self {
        Self {
            monospace: false,
            word_wrap: true,
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

    fn name(&self) -> SpaceViewClassName {
        "Text Document".into()
    }

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
                        // TODO(jleibs): better handling for multiple results
                        if text_document.text_entries.is_empty() {
                            ui.label("No TextDocument entries found.");
                        } else if text_document.text_entries.len() == 1 {
                            let mut text =
                                egui::RichText::new(text_document.text_entries[0].body.as_str());

                            if state.monospace {
                                text = text.monospace();
                            }

                            ui.add(Label::new(text).wrap(state.word_wrap));
                        } else {
                            ui.label(format!(
                                "Unexpected number of text entries: {}. Limit your query to 1.",
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
