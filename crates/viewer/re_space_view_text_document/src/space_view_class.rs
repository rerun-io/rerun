use egui::Label;

use re_space_view::suggest_space_view_for_each_entity;
use re_types::SpaceViewClassIdentifier;
use re_types::View;
use re_ui::UiExt as _;

use re_viewer_context::{
    external::re_log_types::EntityPath, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewState, SpaceViewStateExt as _, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext,
};

use crate::visualizer_system::{TextDocumentEntry, TextDocumentSystem};

// TODO(andreas): This should be a blueprint component.

pub struct TextDocumentSpaceViewState {
    monospace: bool,
    word_wrap: bool,
    commonmark_cache: egui_commonmark::CommonMarkCache,
}

impl Default for TextDocumentSpaceViewState {
    fn default() -> Self {
        Self {
            monospace: false,
            word_wrap: true,
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

type ViewType = re_types::blueprint::views::TextDocumentView;

impl SpaceViewClass for TextDocumentSpaceView {
    fn identifier() -> SpaceViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Text document"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TEXT
    }

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "# Text document view

Displays text from a text component, as raw text or markdown."
            .to_owned()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<TextDocumentSystem>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<TextDocumentSpaceViewState>::default()
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<TextDocumentSpaceViewState>()?;

        ui.selection_grid("text_config").show(ui, |ui| {
            ui.grid_left_hand_label("Text style");
            ui.vertical(|ui| {
                ui.re_radio_value(&mut state.monospace, false, "Proportional");
                ui.re_radio_value(&mut state.monospace, true, "Monospace");
                ui.re_checkbox(&mut state.word_wrap, "Word Wrap");
            });
            ui.end_row();
        });

        Ok(())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();
        // By default spawn a space view for every text document.
        suggest_space_view_for_each_entity::<TextDocumentSystem>(ctx, self)
    }

    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,

        _query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<TextDocumentSpaceViewState>()?;
        let text_document = system_output.view_systems.get::<TextDocumentSystem>()?;

        egui::Frame {
            inner_margin: re_ui::DesignTokens::view_padding().into(),
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

                            if media_type == &re_types::components::MediaType::markdown() {
                                re_tracing::profile_scope!("egui_commonmark");

                                // Make sure headers are big:
                                ui.style_mut()
                                    .text_styles
                                    .entry(egui::TextStyle::Heading)
                                    .or_insert(egui::FontId::proportional(32.0))
                                    .size = 24.0;

                                egui_commonmark::CommonMarkViewer::new()
                                    .max_image_width(Some(ui.available_width().floor() as _))
                                    .show(ui, &mut state.commonmark_cache, body);
                                return;
                            }

                            let mut text = egui::RichText::new(body.as_str());

                            if state.monospace {
                                text = text.monospace();
                            }

                            ui.add(Label::new(text).wrap_mode(if state.word_wrap {
                                egui::TextWrapMode::Wrap
                            } else {
                                egui::TextWrapMode::Extend
                            }));
                        } else {
                            // TODO(jleibs): better handling for multiple results
                            ui.label(format!(
                                "Can only show one text document at a time; was given {}. Update \
                                the query so that it returns a single text document and create \
                                additional views for the others.",
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
