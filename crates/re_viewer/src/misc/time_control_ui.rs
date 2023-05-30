use egui::NumExt as _;

use re_data_store::TimesPerTimeline;
use re_log_types::TimeType;

use super::time_control::{Looping, PlayState, TimeControl};

impl TimeControl {
    pub fn timeline_selector_ui(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        ui: &mut egui::Ui,
    ) {
        self.select_a_valid_timeline(times_per_timeline);

        egui::ComboBox::from_id_source("timeline")
            .selected_text(self.timeline().name().as_str())
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.set_min_width(64.0);

                for timeline in times_per_timeline.timelines() {
                    if ui
                        .selectable_label(timeline == self.timeline(), timeline.name().as_str())
                        .clicked()
                    {
                        self.set_timeline(*timeline);
                    }
                }
            });
    }

    pub fn fps_ui(&mut self, ui: &mut egui::Ui) {
        if self.time_type() == TimeType::Sequence {
            if let Some(mut fps) = self.fps() {
                ui.add(
                    egui::DragValue::new(&mut fps)
                        .suffix(" FPS")
                        .speed(1)
                        .clamp_range(0.0..=f32::INFINITY),
                )
                .on_hover_text("Frames Per Second");
                self.set_fps(fps);
            }
        }
    }

    pub fn play_pause_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        times_per_timeline: &TimesPerTimeline,
        ui: &mut egui::Ui,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0; // from figma
            self.play_button_ui(re_ui, ui, times_per_timeline);
            self.follow_button_ui(re_ui, ui, times_per_timeline);
            self.pause_button_ui(re_ui, ui);
            self.step_time_button_ui(re_ui, ui, times_per_timeline);
            self.loop_button_ui(re_ui, ui);
        });
    }

    fn play_button_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        let is_playing = self.play_state() == PlayState::Playing;
        if re_ui
            .large_button_selected(ui, &re_ui::icons::PLAY, is_playing)
            .on_hover_text(format!("Play.{}", toggle_playback_text(ui.ctx())))
            .clicked()
        {
            self.set_play_state(times_per_timeline, PlayState::Playing);
        }
    }

    fn follow_button_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        let is_following = self.play_state() == PlayState::Following;
        if re_ui
            .large_button_selected(ui, &re_ui::icons::FOLLOW, is_following)
            .on_hover_text(format!(
                "Follow latest data.{}",
                toggle_playback_text(ui.ctx())
            ))
            .clicked()
        {
            self.set_play_state(times_per_timeline, PlayState::Following);
        }
    }

    fn pause_button_ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        let is_paused = self.play_state() == PlayState::Paused;
        if re_ui
            .large_button_selected(ui, &re_ui::icons::PAUSE, is_paused)
            .on_hover_text(format!("Pause.{}", toggle_playback_text(ui.ctx())))
            .clicked()
        {
            self.pause();
        }
    }

    fn step_time_button_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        if re_ui
            .large_button(ui, &re_ui::icons::ARROW_LEFT)
            .on_hover_text("Step back to previous time with any new data (left arrow)")
            .clicked()
        {
            self.step_time_back(times_per_timeline);
        }

        if re_ui
            .large_button(ui, &re_ui::icons::ARROW_RIGHT)
            .on_hover_text("Step forwards to next time with any new data (right arrow)")
            .clicked()
        {
            self.step_time_fwd(times_per_timeline);
        }
    }

    fn loop_button_ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        let icon = &re_ui::icons::LOOP;

        ui.scope(|ui| {
            // Loop-button cycles between states:
            match self.looping() {
                Looping::Off => {
                    if re_ui
                        .large_button_selected(ui, icon, false)
                        .on_hover_text("Looping is off")
                        .clicked()
                    {
                        self.set_looping(Looping::All);
                    }
                }
                Looping::All => {
                    ui.visuals_mut().selection.bg_fill = re_ui::ReUi::loop_everything_color();
                    if re_ui
                        .large_button_selected(ui, icon, true)
                        .on_hover_text("Looping entire recording")
                        .clicked()
                    {
                        self.set_looping(Looping::Selection);
                    }
                }
                Looping::Selection => {
                    // ui.visuals_mut().selection.bg_fill = re_ui::ReUi::loop_selection_color(); // we have one color for the button, and a slightly different shade of it for the actual selection :/
                    #[allow(clippy::collapsible_else_if)]
                    if re_ui
                        .large_button_selected(ui, icon, true)
                        .on_hover_text("Looping selection")
                        .clicked()
                    {
                        self.set_looping(Looping::Off);
                    }
                }
            }
        });
    }

    pub fn playback_speed_ui(&mut self, ui: &mut egui::Ui) {
        let mut speed = self.speed();
        let drag_speed = (speed * 0.02).at_least(0.01);
        ui.add(
            egui::DragValue::new(&mut speed)
                .speed(drag_speed)
                .suffix("x"),
        )
        .on_hover_text("Playback speed.");
        self.set_speed(speed);
    }
}

fn toggle_playback_text(egui_ctx: &egui::Context) -> String {
    // if let Some(shortcut) = re_ui::Command::PlaybackTogglePlayPause.kb_shortcut() {
    //     format!(" Toggle with {}", egui_ctx.format_shortcut(&shortcut))
    // } else {
    //     Default::default()
    // }
    Default::default()
}
