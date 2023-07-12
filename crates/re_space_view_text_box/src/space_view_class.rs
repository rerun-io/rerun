use egui::Label;
use re_viewer_context::{
    external::re_log_types::EntityPath, SpaceViewClass, SpaceViewClassName, SpaceViewId,
    SpaceViewState, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartCollection,
    ViewQuery, ViewerContext,
};

use super::view_part_system::TextBoxSystem;

// TODO(andreas): This should be a blueprint component.
#[derive(Clone, PartialEq, Eq)]
pub struct TextBoxSpaceViewState {
    monospace: bool,
    word_wrap: bool,
}

impl Default for TextBoxSpaceViewState {
    fn default() -> Self {
        Self {
            monospace: false,
            word_wrap: true,
        }
    }
}

impl SpaceViewState for TextBoxSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TextBoxSpaceView;

impl SpaceViewClass for TextBoxSpaceView {
    type State = TextBoxSpaceViewState;

    fn name(&self) -> SpaceViewClassName {
        "Text Box".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TEXTBOX
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi, _state: &Self::State) -> egui::WidgetText {
        "Displays text from a text entry components.".into()
    }

    fn on_register(&self, system_registry: &mut re_viewer_context::SpaceViewSystemRegistry) {
        system_registry.register_part_system::<TextBoxSystem>();
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
                ui.radio_value(&mut state.monospace, false, "Proportional");
                ui.radio_value(&mut state.monospace, true, "Monospace");
                ui.checkbox(&mut state.word_wrap, "Word Wrap");
            });
            ui.end_row();
        });
    }

    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        view_ctx: &ViewContextCollection,
        parts: &ViewPartCollection,
        query: &ViewQuery<'_>,
        draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let text_box = parts.get::<TextBoxSystem>()?;

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
                        if text_box.text_entries.len() == 1 {
                            let mut text = egui::RichText::new(&text_box.text_entries[0].body);

                            if state.monospace {
                                text = text.monospace();
                            }

                            ui.add(Label::new(text).wrap(state.word_wrap));
                        } else {
                            ui.label(format!(
                                "Unexpected number of text entries: {}. Limit your query to 1.",
                                text_box.text_entries.len()
                            ));
                        }
                    })
            })
            .response
        });

        Ok(())
    }
}
