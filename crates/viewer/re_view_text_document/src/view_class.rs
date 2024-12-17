use egui::Label;

use egui::Sense;
use re_types::View;
use re_types::ViewClassIdentifier;
use re_ui::UiExt as _;
use re_view::suggest_view_for_each_entity;

use re_viewer_context::Item;
use re_viewer_context::{
    external::re_log_types::EntityPath, ViewClass, ViewClassRegistryError, ViewId, ViewQuery,
    ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
};

use crate::visualizer_system::{TextDocumentEntry, TextDocumentSystem};

// TODO(andreas): This should be a blueprint component.

pub struct TextDocumentViewState {
    monospace: bool,
    word_wrap: bool,
    commonmark_cache: egui_commonmark::CommonMarkCache,
}

impl Default for TextDocumentViewState {
    fn default() -> Self {
        Self {
            monospace: false,
            word_wrap: true,
            commonmark_cache: Default::default(),
        }
    }
}

impl ViewState for TextDocumentViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TextDocumentView;

type ViewType = re_types::blueprint::views::TextDocumentView;

impl ViewClass for TextDocumentView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Text document"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_TEXT
    }

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "# Text document view

Displays text from a text component, as raw text or markdown."
            .to_owned()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<TextDocumentSystem>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<TextDocumentViewState>::default()
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TextDocumentViewState>()?;

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

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        // By default spawn a view for every text document.
        suggest_view_for_each_entity::<TextDocumentSystem>(ctx, self)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TextDocumentViewState>()?;
        let text_document = system_output.view_systems.get::<TextDocumentSystem>()?;

        let response = egui::Frame {
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
                            ui.error_label(format!(
                                "Can only show one text document at a time; was given {}. Update \
                                the query so that it returns a single text document and create \
                                additional views for the others.",
                                text_document.text_entries.len()
                            ));
                        }
                    })
            })
            .response
        })
        .response
        .interact(Sense::click());

        if response.hovered() {
            ctx.selection_state().set_hovered(Item::View(query.view_id));
        }

        if response.clicked() {
            ctx.selection_state()
                .set_selection(Item::View(query.view_id));
        }

        Ok(())
    }
}
