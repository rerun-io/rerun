mod entrance_animation;
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
use re_viewer_context::AppContext;

const MIN_PAGE_MARGIN: f32 = 16.0;
const MAX_PAGE_MARGIN: f32 = 40.0;

fn page_horizontal_margin(available_width: f32) -> i8 {
    ((available_width - MIN_COLUMN_WIDTH) / 2.0)
        .clamp(MIN_PAGE_MARGIN, MAX_PAGE_MARGIN)
        .round() as i8
}

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
        ctx: &AppContext<'_>,
        welcome_screen_state: &WelcomeScreenState,
        log_sources: &[Arc<LogSource>],
        login_state: &CloudState,
    ) {
        if welcome_screen_state.opacity <= 0.0 {
            return;
        }

        re_tracing::profile_function!();

        // This is needed otherwise `example_page_ui` bleeds by a few pixels over the timeline panel
        // TODO(ab): figure out why that happens
        ui.set_clip_rect(ui.available_rect_before_wrap());

        let horizontal_margin = page_horizontal_margin(ui.available_width());

        let response = egui::ScrollArea::vertical()
            .id_salt("welcome_screen_page")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame {
                    inner_margin: egui::Margin {
                        left: horizontal_margin,
                        right: horizontal_margin,
                        top: 80,
                        bottom: 80,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_margin_preserves_content_on_narrow_screens() {
        assert_eq!(page_horizontal_margin(400.0), 40);
        assert_eq!(page_horizontal_margin(300.0), 25);
        assert_eq!(page_horizontal_margin(250.0), 16);
    }
}
