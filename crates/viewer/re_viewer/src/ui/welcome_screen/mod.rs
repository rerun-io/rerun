mod example_section;
mod intro_section;
mod loading_data_ui;
mod no_data_ui;
mod welcome_section;

use std::sync::Arc;

use example_section::{ExampleSection, MIN_COLUMN_WIDTH};
use re_log_channel::LogSource;

use crate::app_state::WelcomeScreenState;

pub use intro_section::{CloudState, LoginState};
use re_viewer_context::GlobalContext;

#[derive(Default)]
pub struct WelcomeScreen {
    example_page: ExampleSection,
}

impl WelcomeScreen {
    pub fn set_examples_manifest_url(&mut self, egui_ctx: &egui::Context, url: String) {
        self.example_page.set_manifest_url(egui_ctx, url);
    }

    /// Welcome screen shown in place of the viewport when no data is loaded.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &GlobalContext<'_>,
        welcome_screen_state: &WelcomeScreenState,
        log_sources: &[Arc<LogSource>],
        login_state: &CloudState,
    ) {
        if welcome_screen_state.opacity <= 0.0 {
            return;
        }

        // This is needed otherwise `example_page_ui` bleeds by a few pixels over the timeline panel
        // TODO(ab): figure out why that happens
        ui.set_clip_rect(ui.available_rect_before_wrap());

        let horizontal_scroll = ui.available_width() < 40.0 * 2.0 + MIN_COLUMN_WIDTH;

        let response = egui::ScrollArea::new([horizontal_scroll, true])
            .id_salt("welcome_screen_page")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame {
                    inner_margin: egui::Margin {
                        left: 40,
                        right: 40,
                        top: 50,
                        bottom: 8,
                    },
                    ..Default::default()
                }
                .show(ui, |ui| {
                    if welcome_screen_state.hide_examples {
                        if let Some(loading_text) =
                            loading_data_ui::loading_text_for_data_sources(log_sources)
                        {
                            loading_data_ui::loading_data_ui(ui, &loading_text);
                        } else {
                            no_data_ui::no_data_ui(ui);
                        }
                    } else {
                        self.example_page.ui(ui, ctx, login_state);
                    }
                });
            });

        if welcome_screen_state.opacity < 1.0 {
            let cover_opacity = 1.0 - welcome_screen_state.opacity;
            let fill_color = ui.visuals().panel_fill.gamma_multiply(cover_opacity);
            ui.painter()
                .rect_filled(response.inner_rect, 0.0, fill_color);
        }
    }
}
