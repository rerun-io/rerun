use egui::{Label, Sense};
use re_sdk_types::blueprint::archetypes::TextDocumentFormat;
use re_sdk_types::blueprint::components::Enabled;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, UiExt as _};
use re_viewer_context::external::re_log_types::EntityPath;
use re_viewer_context::{
    IdentifiedViewSystem as _, Item, SystemCommand, SystemCommandSender as _, ViewClass,
    ViewClassExt as _, ViewClassRegistryError, ViewId, ViewQuery, ViewState, ViewStateExt as _,
    ViewSystemExecutionError, ViewerContext, suggest_view_for_each_entity,
};
use re_viewport_blueprint::ViewProperty;

use crate::visualizer_system::{TextDocumentEntry, TextDocumentSystem};

#[derive(Default)]
pub struct TextDocumentViewState {
    commonmark_cache: egui_commonmark::CommonMarkCache,
    only_showing_markdown: bool,
}

impl re_byte_size::SizeBytes for TextDocumentViewState {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            commonmark_cache,
            only_showing_markdown: _,
        } = self;
        // Most of the memory not tracked unfortunately.
        commonmark_cache.link_hooks().heap_size_bytes()
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

type ViewType = re_sdk_types::blueprint::views::TextDocumentView;

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

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Text document view")
            .docs_link("https://rerun.io/docs/reference/types/views/text_document_view")
            .markdown("Supports raw text and markdown.")
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_fallback_provider(
            TextDocumentFormat::descriptor_word_wrap().component,
            |_ctx| Enabled::from(true),
        );

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
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_ref::<TextDocumentViewState>()?;

        if !state.only_showing_markdown {
            ui.list_item_scope("text_document_selection_ui", |ui| {
                let ctx = self.view_context(ctx, view_id, state, space_origin);
                re_view::view_property_ui::<TextDocumentFormat>(&ctx, ui);
            });
        }

        Ok(())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        // By default spawn a view for every text document.
        suggest_view_for_each_entity::<TextDocumentSystem>(ctx, include_entity)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        let tokens = ui.tokens();
        let state = state.downcast_mut::<TextDocumentViewState>()?;
        let text_entries = system_output
            .visualizer_data::<Vec<TextDocumentEntry>>(TextDocumentSystem::identifier())?;
        state.only_showing_markdown = !text_entries.is_empty()
            && text_entries
                .iter()
                .all(|entry| entry.media_type == re_sdk_types::components::MediaType::markdown());

        let frame = egui::Frame::new().inner_margin(tokens.view_padding());
        let (response, text_document_result) = frame
            .show(ui, |ui| {
                let inner_ui_builder = egui::UiBuilder::new()
                    .layout(egui::Layout::top_down(egui::Align::LEFT))
                    .sense(Sense::click());
                ui.scope_builder(inner_ui_builder, |ui| {
                    let text_document_result = egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            text_document_ui(ctx, query, ui, state, text_entries)
                        })
                        .inner;

                    (ui.response(), text_document_result)
                })
                .inner
            })
            .inner;
        text_document_result?;

        // Since we want the view to be hoverable / clickable when the pointer is over a label
        // (and we want selectable labels), we need to work around egui's interactions here.
        // Since `rect_contains_pointer` checks for the layer id, this shouldn't cause any problems
        // with popups / modals.
        let hovered = ui.ctx().rect_contains_pointer(ui.layer_id(), response.rect);
        let clicked = hovered && ui.input(|i| i.pointer.primary_pressed());

        if hovered {
            ctx.selection_state().set_hovered(Item::View(query.view_id));
        }

        if clicked {
            ctx.command_sender()
                .send_system(SystemCommand::set_selection(Item::View(query.view_id)));
        }

        Ok(())
    }
}

fn text_document_ui(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    ui: &mut egui::Ui,
    state: &mut TextDocumentViewState,
    text_entries: &[TextDocumentEntry],
) -> Result<(), ViewSystemExecutionError> {
    let view_ctx = TextDocumentView.view_context(ctx, query.view_id, state, query.space_origin);
    let format_property = ViewProperty::from_archetype::<TextDocumentFormat>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        query.view_id,
    );
    let monospace = format_property.component_or_fallback::<Enabled>(
        &view_ctx,
        TextDocumentFormat::descriptor_monospace().component,
    )?;
    let word_wrap = format_property.component_or_fallback::<Enabled>(
        &view_ctx,
        TextDocumentFormat::descriptor_word_wrap().component,
    )?;

    if text_entries.is_empty() {
        // We get here if we scroll back time to before the first text document was logged.
        ui.weak("(empty)");
    } else if text_entries.len() == 1 {
        let TextDocumentEntry { body, media_type } = &text_entries[0];

        if media_type == &re_sdk_types::components::MediaType::markdown() {
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
        } else {
            let mut text = egui::RichText::new(body.as_str());

            if **monospace {
                text = text.monospace();
            }

            ui.add(Label::new(text).wrap_mode(if **word_wrap {
                egui::TextWrapMode::Wrap
            } else {
                egui::TextWrapMode::Extend
            }));
        }
    } else {
        // TODO(jleibs): better handling for multiple results
        ui.error_label(format!(
            "Can only show one text document at a time; was given {}. Update \
                                    the query so that it returns a single text document and create \
                                    additional views for the others.",
            text_entries.len()
        ));
    }

    Ok(())
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| TextDocumentView.help(ctx));
}
