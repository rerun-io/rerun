use egui::{NumExt as _, Popup, RectAlign};
use re_entity_db::EntityDb;
use re_log_types::TimeType;
use re_sdk_types::blueprint::components::{LoopMode, PlayState};
use re_ui::menu::menu_style;
use re_ui::{
    ComboItem, ReButton, RecordingCommandKind, Size, UiExt as _, Variant, icons, list_item,
};
use re_viewer_context::{TimeControl, TimeControlCommand};

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct TimeControlUi;

const TIME_CONTROL_ROW_SIZE: Size = Size::custom(22.0);

impl TimeControlUi {
    #[expect(clippy::unused_self)]
    pub fn timeline_selector_ui(
        &self,
        time_ctrl: &TimeControl,
        entity_db: &EntityDb,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let response = ui
            .add(
                ReButton::dropdown(time_ctrl.timeline_name().as_str())
                    .size(TIME_CONTROL_ROW_SIZE)
                    .ghost(),
            )
            .on_hover_ui(|ui| {
                list_item::list_item_scope(ui, "tooltip", |ui| {
                    ui.markdown_ui(
                        r"
Select timeline.

Each piece of logged data is associated with one or more timelines.

The logging SDK can create two timelines for you automatically:
* `log_time` - a temporal timeline with the time of the log call (opt-out)
* `log_tick` - a sequence timeline with the sequence number of the log call (opt-in)

You can also define your own timelines, e.g. for sensor time or camera frame number.
"
                        .trim(),
                    );

                    ui.re_hyperlink(
                        "Full documentation",
                        "https://rerun.io/docs/concepts/logging-and-ingestion/timelines",
                        // Always open in a new tab
                        true,
                    );
                });
            });
        Popup::menu(&response).style(menu_style()).show(|ui| {
            let timelines = entity_db.timelines();

            if timelines.is_empty() {
                ui.weak("The recording has no timelines");
                return;
            }

            for timeline in timelines.values() {
                let num_rows = entity_db.num_temporal_rows_on_timeline(timeline.name());
                if ui
                    .add(
                        ComboItem::new(timeline.name().as_str())
                            .value(format!("{} rows", re_format::format_uint(num_rows)))
                            .selected(timeline.name() == time_ctrl.timeline_name()),
                    )
                    .clicked()
                {
                    time_commands.push(TimeControlCommand::SetActiveTimeline(*timeline.name()));
                }
            }
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
            ReButton::wrap_widget(ui, Variant::Ghost, TIME_CONTROL_ROW_SIZE, false, |ui| {
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
            self.play_pause_button_ui(time_ctrl, ui, time_commands);
            self.playhead_nav_ui(ui, time_commands);
            self.loop_button_ui(time_ctrl, ui, time_commands);
        });
    }

    #[expect(clippy::unused_self)]
    fn play_pause_button_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let is_paused = time_ctrl.play_state() == PlayState::Paused;
        if ui
            .add(
                ReButton::icon(if is_paused { icons::PLAY } else { icons::PAUSE })
                    .selected(!is_paused)
                    .size(TIME_CONTROL_ROW_SIZE)
                    .secondary(),
            )
            .on_hover_ui(|ui| RecordingCommandKind::PlaybackTogglePlayPause.tooltip_ui(ui))
            .clicked()
        {
            time_commands.push(TimeControlCommand::TogglePlayPause);
        }
    }

    #[expect(clippy::unused_self)]
    fn playhead_nav_ui(&self, ui: &mut egui::Ui, time_commands: &mut Vec<TimeControlCommand>) {
        let commands = [
            [
                RecordingCommandKind::PlaybackForward,
                RecordingCommandKind::PlaybackBack,
            ],
            [
                RecordingCommandKind::PlaybackForwardFast,
                RecordingCommandKind::PlaybackBackFast,
            ],
            [
                RecordingCommandKind::PlaybackStepForward,
                RecordingCommandKind::PlaybackStepBack,
            ],
            [
                RecordingCommandKind::PlaybackEndAndFollow,
                RecordingCommandKind::PlaybackBeginning,
            ],
        ];

        let tokens = ui.tokens();

        // Keep the button looking hovered while its menu popup is open.
        let popup_id = ui.id().with("playhead_nav_menu");
        let popup_open = egui::Popup::is_id_open(ui.ctx(), popup_id);

        // Match the height of a `large_button`.
        let button = ui
            .scope(|ui| {
                ui.spacing_mut().interact_size.y = tokens.large_button_size.y;

                ui.add(
                    re_ui::ReButton::new((
                        re_ui::icons::PLAYHEAD_NAV,
                        re_ui::icons::DROPDOWN_ARROW,
                    ))
                    .secondary()
                    .size(TIME_CONTROL_ROW_SIZE)
                    .highlighted(popup_open),
                )
            })
            .inner;

        egui::Popup::menu(&button)
            .style(menu_style())
            .id(popup_id)
            .align(RectAlign::TOP_START)
            .show(|ui| {
                for (idx, group) in commands.into_iter().enumerate() {
                    if idx > 0 {
                        ui.separator();
                    }

                    for command in group {
                        let button = command.menu_button(ui.ctx());
                        let button = ui.add(button).on_hover_ui(|ui| command.tooltip_ui(ui));

                        if button.clicked()
                            && let Some(time_command) =
                                TimeControlCommand::from_recording_command(command)
                        {
                            time_commands.push(time_command);
                        }
                    }
                }
            });
    }

    #[expect(clippy::unused_self)]
    fn loop_button_ui(
        &self,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let button = ReButton::icon(re_ui::icons::LOOP)
            .size(TIME_CONTROL_ROW_SIZE)
            .secondary();

        ui.scope(|ui| {
            // Loop-button cycles between states:
            match time_ctrl.loop_mode() {
                LoopMode::Off => {
                    if ui.add(button).on_hover_text("Looping is off").clicked() {
                        time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::All));
                    }
                }
                LoopMode::All => {
                    ui.visuals_mut().selection.bg_fill = ui.tokens().loop_everything_color;
                    if ui
                        .add(button.selected(true))
                        .on_hover_text("Looping is off")
                        .clicked()
                    {
                        // Only go to the selection time selection mode if there's already a selection.
                        // (otherwise, we'd create a selection as a fail-safe, but that's rather confusing!)
                        if time_ctrl.time_selection().is_some() {
                            time_commands
                                .push(TimeControlCommand::SetLoopMode(LoopMode::Selection));
                        } else {
                            time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::Off));
                        }
                    }
                }
                LoopMode::Selection => {
                    // No need for this - the selection color is already same as the loop color.
                    // ui.visuals_mut().selection.bg_fill = ui.tokens().loop_selection_color.to_opaque();

                    if ui
                        .add(button.selected(true))
                        .on_hover_text("Looping is off")
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

        ReButton::wrap_widget(ui, Variant::Ghost, TIME_CONTROL_ROW_SIZE, false, |ui| {
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
