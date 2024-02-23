use web_time::Instant;

use re_data_store::{LatestAtQuery, TimeType};
use re_space_view::suggest_space_view_for_each_entity;
use re_viewer_context::external::{re_entity_db::EntityProperties, re_log_types::RowId};
use re_viewer_context::{
    external::re_log_types::EntityPath, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewState, SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
};

use crate::{
    audio_player::{StereoAudio, AUDIO_PLAYER},
    visualizer_system::{AudioEntry, AudioSystem},
};

pub struct AudioSpaceViewState {
    volume: f32,

    scrubbing: bool,

    /// For each entity, which is the currently playing audio, and at which frame offset?
    last_played_audio: nohash_hasher::IntMap<EntityPath, (RowId, usize)>,

    last_time: Instant,
}

impl Default for AudioSpaceViewState {
    fn default() -> Self {
        Self {
            volume: 1.0,
            scrubbing: false,
            last_played_audio: Default::default(),
            last_time: Instant::now(),
        }
    }
}

impl SpaceViewState for AudioSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct AudioSpaceView;

impl SpaceViewClass for AudioSpaceView {
    type State = AudioSpaceViewState;

    const IDENTIFIER: &'static str = "Audio";
    const DISPLAY_NAME: &'static str = "Audio";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TEXT
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "Shows and plays audio.".into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<AudioSystem>()
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
        let Self::State {
            volume, scrubbing, ..
        } = state;

        ctx.re_ui.selection_grid(ui, "text_config").show(ui, |ui| {
            ctx.re_ui.grid_left_hand_label(ui, "Volume");
            ui.vertical(|ui| {
                ui.add(egui::DragValue::new(volume).speed(0.05));
            });
            ui.end_row();

            ctx.re_ui
                .grid_left_hand_label(ui, "Enable scrubbing (very buggy!)");
            ui.checkbox(scrubbing, "");
            ui.end_row();
        });
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();
        // By default spawn a space view for every Audio.
        suggest_space_view_for_each_entity::<AudioSystem>(ctx, self)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        _query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let audio = system_output.view_systems.get::<AudioSystem>()?;

        let is_playing = ctx.rec_cfg.time_ctrl.read().is_playing();
        let is_scrubbing = state.scrubbing
            && !is_playing
            && ctx.rec_cfg.time_ctrl.read().time_type() == TimeType::Time;

        if is_scrubbing {
            AUDIO_PLAYER.stop(); // TODO: we can't do this here, because there might be multiple audio space views.
        }
        let now = Instant::now();
        let elapsed = now - state.last_time;
        state.last_time = now;
        // let dt = elapsed.as_secs_f32();
        let dt = ui.input(|i| i.unstable_dt);

        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if audio.entries.is_empty() {
                            // We get here if we scroll back time to before the first Audio was logged.
                            ui.weak("(empty)");
                        } else {
                            for entry in &audio.entries {
                                if let Some(query) = &audio.query {
                                    audio_entry_ui(ctx, ui, query, entry);
                                    handle_entry(ctx, query, is_scrubbing, state, dt, entry);
                                }
                            }
                        }
                    })
            })
            .response
        });

        Ok(())
    }
}

fn handle_entry(
    ctx: &ViewerContext<'_>,
    query: &LatestAtQuery,
    is_scrubbing: bool,
    state: &mut AudioSpaceViewState,
    dt: f32,
    entry: &AudioEntry,
) {
    let mut audio = match StereoAudio::try_from(entry) {
        Ok(audio) => audio,
        Err(err) => {
            re_log::warn_once!("{err}");
            return;
        }
    };

    let AudioEntry {
        row_id,
        entity_path,
        data_time,
        duration_sec,
        ..
    } = entry;

    if is_scrubbing {
        if query.timeline.typ() != TimeType::Time {
            return;
        }
        let Some(data_time) = data_time else {
            return;
        };
        let ns_since_logged = query.at - *data_time;
        let time_offset = 1e-9 * ns_since_logged.as_f64();
        let frame_offset = (time_offset.max(0.0) * audio.frame_rate as f64).round() as usize;

        let n = audio.frames.len();

        if let Some((last_row_id, last_frame_offset)) =
            state.last_played_audio.get(entity_path).copied()
        {
            if last_row_id == *row_id {
                // scrub!
                // We enqueue more than we need to avoid audio glitches,
                // then clear it next frame.
                let buffer_factor = 4;

                match last_frame_offset.cmp(&frame_offset) {
                    std::cmp::Ordering::Less => {
                        // time is moving forwards
                        let count = buffer_factor * (frame_offset - last_frame_offset);
                        let end = (last_frame_offset + count).min(n);
                        audio.frames = audio.frames[last_frame_offset.min(n)..end].to_vec();
                        audio.frame_rate = (frame_offset - last_frame_offset) as f32 / dt;
                    }
                    std::cmp::Ordering::Equal => {
                        audio = Default::default(); // time is still
                    }
                    std::cmp::Ordering::Greater => {
                        // time is moving backwards
                        let count = buffer_factor * (last_frame_offset - frame_offset);
                        let begin = last_frame_offset.saturating_sub(count);
                        audio.frames = audio.frames[begin..last_frame_offset.min(n)].to_vec();
                        audio.frames.reverse();
                        audio.frame_rate = (last_frame_offset - frame_offset) as f32 / dt;
                    }
                }

                if !audio.is_empty() {
                    AUDIO_PLAYER.play(audio);
                }
            } else {
                audio.frames.drain(0..frame_offset.min(n));
                AUDIO_PLAYER.play(audio);
            }
        } else {
            audio.frames.drain(0..frame_offset.min(n));
            AUDIO_PLAYER.play(audio);
        }

        state
            .last_played_audio
            .insert(entity_path.clone(), (*row_id, frame_offset));
    } else if ctx.rec_cfg.time_ctrl.read().is_playing() {
        let last_played_row_id = state
            .last_played_audio
            .get(entity_path)
            .map(|(row_id, _)| row_id);
        if last_played_row_id != Some(row_id) {
            let mut offset_sec = 0.0;
            if let (Some(data_time), Some(duration_sec)) = (data_time, duration_sec) {
                if query.timeline.typ() == TimeType::Time {
                    let ns_since_logged = query.at - *data_time;
                    let sec_since_logged = 1e-9 * ns_since_logged.as_f64();
                    if 0.0 <= sec_since_logged && sec_since_logged <= *duration_sec {
                        offset_sec = sec_since_logged;
                    } else {
                        return; // Don't play - we're outside the duration
                    }
                }
            }

            let frame_offset = (offset_sec.max(0.0) * audio.frame_rate as f64).round() as usize;
            audio.frames.drain(0..frame_offset);

            AUDIO_PLAYER.play(audio);

            state
                .last_played_audio
                .insert(entity_path.clone(), (*row_id, frame_offset));
        }
    } else {
        AUDIO_PLAYER.stop();
        state.last_played_audio = Default::default();

        // TODO: this could be useful
        // if ui.button("Play").clicked() {
        //     AUDIO_PLAYER.play(entry);
        // }
    }
}

fn audio_entry_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query: &LatestAtQuery,
    audio_entry: &AudioEntry,
) {
    use re_ui::SyntaxHighlighting as _;

    let re_ui = ctx.re_ui;

    let timeline = query.timeline;

    let AudioEntry {
        row_id,
        entity_path,
        data_time,
        data: _,
        frame_rate,
        num_channels,
        num_frames,
        duration_sec,
    } = audio_entry;

    egui::Grid::new("audio_info").num_columns(2).show(ui, |ui| {
        re_ui.grid_left_hand_label(ui, "ID");
        ui.label(row_id.to_string());
        ui.end_row();

        re_ui.grid_left_hand_label(ui, "Entity");
        ui.label(entity_path.syntax_highlighted(ui.style()));
        ui.end_row();

        if let Some(data_time) = *data_time {
            re_ui.grid_left_hand_label(ui, "Logged at");
            ui.label(timeline.typ().format(data_time, ctx.app_options.time_zone));
            ui.end_row();
        }

        re_ui.grid_left_hand_label(ui, "Sample rate");
        ui.label(format!(
            "{} Hz",
            re_format::format_number((*frame_rate) as _)
        ));
        ui.end_row();

        if let Some(channels) = num_channels {
            re_ui.grid_left_hand_label(ui, "Channels");
            ui.label(channels.to_string());
            ui.end_row();
        }

        if let Some(num_frames) = num_frames {
            re_ui.grid_left_hand_label(ui, "Frames");
            ui.label(re_format::format_number(*num_frames as _));
            ui.end_row();
        }

        if let Some(duration_sec) = duration_sec {
            re_ui.grid_left_hand_label(ui, "Duration");
            ui.label(format!("{duration_sec:.3} s"));
            ui.end_row();
        }

        if let Some(data_time) = *data_time {
            if timeline.typ() == TimeType::Time {
                let ns_since_logged = query.at - data_time;
                let sec_since_logged = 1e-9 * ns_since_logged.as_f64();
                if let Some(duration_sec) = duration_sec {
                    if 0.0 <= sec_since_logged && sec_since_logged <= *duration_sec {
                        re_ui.grid_left_hand_label(ui, "Current time");
                        ui.label(format!("{sec_since_logged:.3} s"));
                        ui.end_row();
                    }
                }
            }
        }
    });
}
