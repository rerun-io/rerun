use egui::NumExt as _;

use re_data_store::TimesPerTimeline;
use re_log_types::TimeType;

use re_viewer_context::{Looping, PlayState, TimeControl};

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct TimeControlUi;

impl TimeControlUi {
    #[allow(clippy::unused_self)]
    pub fn timeline_selector_ui(
        &mut self,
        time_control: &mut TimeControl,
        times_per_timeline: &TimesPerTimeline,
        ui: &mut egui::Ui,
    ) {
        time_control.select_a_valid_timeline(times_per_timeline);

        egui::ComboBox::from_id_source("timeline")
            .selected_text(time_control.timeline().name().as_str())
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.set_min_width(64.0);

                for timeline in times_per_timeline.timelines() {
                    if ui
                        .selectable_label(
                            timeline == time_control.timeline(),
                            timeline.name().as_str(),
                        )
                        .clicked()
                    {
                        time_control.set_timeline(*timeline);
                    }
                }
            });
    }

    #[allow(clippy::unused_self)]
    pub fn fps_ui(&mut self, time_control: &mut TimeControl, ui: &mut egui::Ui) {
        if time_control.time_type() == TimeType::Sequence {
            if let Some(mut fps) = time_control.fps() {
                ui.add(
                    egui::DragValue::new(&mut fps)
                        .suffix(" FPS")
                        .speed(1)
                        .clamp_range(0.0..=f32::INFINITY),
                )
                .on_hover_text("Frames Per Second");
                time_control.set_fps(fps);
            }
        }
    }

    pub fn play_pause_ui(
        &mut self,
        time_control: &mut TimeControl,
        re_ui: &re_ui::ReUi,
        times_per_timeline: &TimesPerTimeline,
        ui: &mut egui::Ui,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0; // from figma
            self.play_button_ui(time_control, re_ui, ui, times_per_timeline);
            self.follow_button_ui(time_control, re_ui, ui, times_per_timeline);
            self.pause_button_ui(time_control, re_ui, ui);
            self.step_time_button_ui(time_control, re_ui, ui, times_per_timeline);
            self.loop_button_ui(time_control, re_ui, ui);
        });
    }

    #[allow(clippy::unused_self)]
    fn play_button_ui(
        &mut self,
        time_control: &mut TimeControl,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        let is_playing = time_control.play_state() == PlayState::Playing;
        if re_ui
            .large_button_selected(ui, &re_ui::icons::PLAY, is_playing)
            .on_hover_text(format!("Play.{}", toggle_playback_text(ui.ctx())))
            .clicked()
        {
            time_control.set_play_state(times_per_timeline, PlayState::Playing);
        }
    }

    #[allow(clippy::unused_self)]
    fn follow_button_ui(
        &mut self,
        time_control: &mut TimeControl,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        let is_following = time_control.play_state() == PlayState::Following;
        if re_ui
            .large_button_selected(ui, &re_ui::icons::FOLLOW, is_following)
            .on_hover_text(format!(
                "Follow latest data.{}",
                toggle_playback_text(ui.ctx())
            ))
            .clicked()
        {
            time_control.set_play_state(times_per_timeline, PlayState::Following);
        }
    }

    #[allow(clippy::unused_self)]
    fn pause_button_ui(
        &mut self,
        time_control: &mut TimeControl,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
    ) {
        let is_paused = time_control.play_state() == PlayState::Paused;
        if re_ui
            .large_button_selected(ui, &re_ui::icons::PAUSE, is_paused)
            .on_hover_text(format!("Pause.{}", toggle_playback_text(ui.ctx())))
            .clicked()
        {
            time_control.pause();
        }
    }

    #[allow(clippy::unused_self)]
    fn step_time_button_ui(
        &mut self,
        time_control: &mut TimeControl,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        if re_ui
            .large_button(ui, &re_ui::icons::ARROW_LEFT)
            .on_hover_text("Step back to previous time with any new data (left arrow)")
            .clicked()
        {
            time_control.step_time_back(times_per_timeline);
        }

        if re_ui
            .large_button(ui, &re_ui::icons::ARROW_RIGHT)
            .on_hover_text("Step forwards to next time with any new data (right arrow)")
            .clicked()
        {
            time_control.step_time_fwd(times_per_timeline);
        }
    }

    #[allow(clippy::unused_self)]
    fn loop_button_ui(
        &mut self,
        time_control: &mut TimeControl,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
    ) {
        let icon = &re_ui::icons::LOOP;

        ui.scope(|ui| {
            // Loop-button cycles between states:
            match time_control.looping() {
                Looping::Off => {
                    if re_ui
                        .large_button_selected(ui, icon, false)
                        .on_hover_text("Looping is off")
                        .clicked()
                    {
                        time_control.set_looping(Looping::All);
                    }
                }
                Looping::All => {
                    ui.visuals_mut().selection.bg_fill = re_ui::ReUi::loop_everything_color();
                    if re_ui
                        .large_button_selected(ui, icon, true)
                        .on_hover_text("Looping entire recording")
                        .clicked()
                    {
                        time_control.set_looping(Looping::Selection);
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
                        time_control.set_looping(Looping::Off);
                    }
                }
            }
        });
    }

    #[allow(clippy::unused_self)]
    pub fn playback_speed_ui(&mut self, time_control: &mut TimeControl, ui: &mut egui::Ui) {
        let mut speed = time_control.speed();
        let drag_speed = (speed * 0.02).at_least(0.01);
        ui.add(
            egui::DragValue::new(&mut speed)
                .speed(drag_speed)
                .suffix("x"),
        )
        .on_hover_text("Playback speed.");
        time_control.set_speed(speed);
    }
}

fn toggle_playback_text(egui_ctx: &egui::Context) -> String {
    if let Some(shortcut) = re_ui::Command::PlaybackTogglePlayPause.kb_shortcut() {
        format!(" Toggle with {}", egui_ctx.format_shortcut(&shortcut))
    } else {
        Default::default()
    }
}
