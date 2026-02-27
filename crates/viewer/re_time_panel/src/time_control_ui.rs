use egui::NumExt as _;
use re_entity_db::TimeHistogramPerTimeline;
use re_log_types::TimeType;
use re_sdk_types::blueprint::components::{LoopMode, PlayState};
use re_ui::{UICommand, UiExt as _, list_item};
use re_viewer_context::{TimeControl, TimeControlCommand};

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct TimeControlUi;

impl TimeControlUi {
    #[expect(clippy::unused_self)]
    pub fn timeline_selector_ui(
        &self,
        time_ctrl: &TimeControl,
        timeline_histograms: &TimeHistogramPerTimeline,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        ui.scope(|ui| {
            ui.spacing_mut().button_padding += egui::Vec2::new(2.0, 0.0);
            ui.visuals_mut().widgets.active.expansion = 0.0;
            ui.visuals_mut().widgets.hovered.expansion = 0.0;
            ui.visuals_mut().widgets.open.expansion = 0.0;

            let response = egui::ComboBox::from_id_salt("timeline")
                .selected_text(time_ctrl.timeline_name().as_str())
                .show_ui(ui, |ui| {
                    for histogram in timeline_histograms.histograms() {
                        let timeline = &histogram.timeline();
                        if ui
                            .selectable_label(
                                timeline.name() == time_ctrl.timeline_name(),
                                (
                                    timeline.name().as_str(),
                                    egui::Atom::grow(),
                                    egui::RichText::new(format!(
                                        "{} rows",
                                        re_format::format_uint(histogram.num_rows())
                                    ))
                                    .size(10.0)
                                    .color(ui.tokens().text_subdued),
                                ),
                            )
                            .clicked()
                        {
                            time_commands
                                .push(TimeControlCommand::SetActiveTimeline(*timeline.name()));
                        }
                    }
                })
                .response
                .on_hover_ui(|ui| {
                    list_item::list_item_scope(ui, "tooltip", |ui| {
                        ui.markdown_ui(
                            r"
Select timeline.

Each piece of logged data is associated with one or more timelines.

The logging SDK always creates two timelines for you:
* `log_tick` - a sequence timeline with the sequence number of the log call
* `log_time` - a temporal timeline with the time of the log call

You can also define your own timelines, e.g. for sensor time or camera frame number.
"
                            .trim(),
                        );

                        ui.re_hyperlink(
                            "Full documentation",
                            "https://rerun.io/docs/concepts/timelines",
                            // Always open in a new tab
                            true,
                        );
                    });
                });
            // Sort of an inline of the `egui::Response::context_menu` function.
            // This is required to assign an id to the context menu, which would
            // otherwise conflict with the popup of this `ComboBox`'s popup menu.
            egui::Popup::menu(&response)
                .id(egui::Id::new("timeline select context menu"))
                .open_memory(if response.secondary_clicked() {
                    Some(egui::SetOpenCommand::Bool(true))
                } else if response.clicked() {
                    // Explicitly close the menu if the widget was clicked
                    // Without this, the context menu would stay open if the user clicks the widget
                    Some(egui::SetOpenCommand::Bool(false))
                } else {
                    None
                })
                .at_pointer_fixed()
                .show(|ui| {
                    if ui.button("Copy timeline name").clicked() {
                        let timeline = format!("{}", time_ctrl.timeline_name());
                        re_log::info!("Copied timeline: {}", timeline);
                        ui.copy_text(timeline);
                    }
                })
        });
    }

    #[expect(clippy::unused_self)]
    pub fn fps_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        if time_ctrl.time_type() == Some(TimeType::Sequence)
            && let Some(mut fps) = time_ctrl.fps()
        {
            let old_fps = fps;
            ui.scope(|ui| {
                ui.spacing_mut().interact_size -= egui::Vec2::new(0., 4.);

                ui.add(
                    egui::DragValue::new(&mut fps)
                        .suffix(" FPS")
                        .speed(1)
                        .range(0.0..=f32::INFINITY),
                )
                .on_hover_text("Frames per second");
            });
            if old_fps != fps {
                time_commands.push(TimeControlCommand::SetFps(fps));
            }
        }
    }

    pub fn play_pause_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0; // from figma
            self.play_button_ui(time_ctrl, ui, time_commands);
            self.follow_button_ui(time_ctrl, ui, time_commands);
            self.pause_button_ui(time_ctrl, ui, time_commands);
            self.step_time_button_ui(ui, time_commands);
            self.loop_button_ui(time_ctrl, ui, time_commands);
        });
    }

    #[expect(clippy::unused_self)]
    fn play_button_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let is_playing = time_ctrl.play_state() == PlayState::Playing;
        if ui
            .large_button_selected(&re_ui::icons::PLAY, is_playing)
            .on_hover_ui(|ui| UICommand::PlaybackTogglePlayPause.tooltip_ui(ui))
            .clicked()
        {
            if matches!(time_ctrl.play_state(), PlayState::Playing) {
                time_commands.push(TimeControlCommand::SetPlayState(PlayState::Paused));
            } else {
                time_commands.push(TimeControlCommand::SetPlayState(PlayState::Playing));
            }
        }
    }

    #[expect(clippy::unused_self)]
    fn follow_button_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let is_following = time_ctrl.play_state() == PlayState::Following;
        if ui
            .large_button_selected(&re_ui::icons::FOLLOW, is_following)
            .on_hover_ui(|ui| UICommand::PlaybackFollow.tooltip_ui(ui))
            .clicked()
        {
            time_commands.push(TimeControlCommand::SetPlayState(PlayState::Following));
        }
    }

    #[expect(clippy::unused_self)]
    fn pause_button_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let is_paused = time_ctrl.play_state() == PlayState::Paused;
        if ui
            .large_button_selected(&re_ui::icons::PAUSE, is_paused)
            .on_hover_ui(|ui| UICommand::PlaybackTogglePlayPause.tooltip_ui(ui))
            .clicked()
        {
            time_commands.push(TimeControlCommand::TogglePlayPause);
        }
    }

    #[expect(clippy::unused_self)]
    fn step_time_button_ui(&self, ui: &mut egui::Ui, time_commands: &mut Vec<TimeControlCommand>) {
        if ui
            .large_button(&re_ui::icons::ARROW_LEFT)
            .on_hover_ui(|ui| UICommand::PlaybackStepBack.tooltip_ui(ui))
            .clicked()
        {
            time_commands.push(TimeControlCommand::StepTimeBack);
        }

        if ui
            .large_button(&re_ui::icons::ARROW_RIGHT)
            .on_hover_ui(|ui| UICommand::PlaybackStepForward.tooltip_ui(ui))
            .clicked()
        {
            time_commands.push(TimeControlCommand::StepTimeForward);
        }
    }

    #[expect(clippy::unused_self)]
    fn loop_button_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let icon = &re_ui::icons::LOOP;

        ui.scope(|ui| {
            // Loop-button cycles between states:
            match time_ctrl.loop_mode() {
                LoopMode::Off => {
                    if ui
                        .large_button_selected(icon, false)
                        .on_hover_text("Looping is off")
                        .clicked()
                    {
                        time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::All));
                    }
                }
                LoopMode::All => {
                    ui.visuals_mut().selection.bg_fill = ui.tokens().loop_everything_color;
                    if ui
                        .large_button_selected(icon, true)
                        .on_hover_text("Looping entire recording")
                        .clicked()
                    {
                        time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::Selection));
                    }
                }
                LoopMode::Selection => {
                    // No need for this - the selection color is already same as the loop color.
                    // ui.visuals_mut().selection.bg_fill = ui.tokens().loop_selection_color.to_opaque();

                    if ui
                        .large_button_selected(icon, true)
                        .on_hover_text("Looping selection")
                        .clicked()
                    {
                        time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::Off));
                    }
                }
            }
        });
    }

    #[expect(clippy::unused_self)]
    pub fn playback_speed_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let mut speed = time_ctrl.speed();
        let drag_speed = (speed * 0.02).at_least(0.01);
        ui.scope(|ui| {
            ui.spacing_mut().interact_size -= egui::Vec2::new(0., 4.);
            ui.add(
                egui::DragValue::new(&mut speed)
                    .speed(drag_speed)
                    .suffix("x"),
            )
            .on_hover_text("Playback speed");
        });

        if speed != time_ctrl.speed() {
            time_commands.push(TimeControlCommand::SetSpeed(speed));
        }
    }
}
