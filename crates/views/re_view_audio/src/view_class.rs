use std::sync::Arc;

use egui::{Rangef, Sense};
use re_audio::{AudioBuffer, OutputError, StreamId, StreamRequest};
use re_log_types::hash::Hash64;
use re_log_types::{AbsoluteTimeRange, TimeReal, TimeType, TimelineName};
use re_sdk_types::blueprint::archetypes::AudioPlayback;
use re_sdk_types::blueprint::components::{PlayState, Volume};
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, IconText, UiExt as _, icons};
use re_viewer_context::external::nohash_hasher::IntMap;
use re_viewer_context::external::re_log_types::EntityPath;
use re_viewer_context::{
    IdentifiedViewSystem as _, Item, SystemCommand, SystemCommandSender as _, TimeControl,
    TimeControlCommand, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewId, ViewQuery,
    ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext, ViewerDiagnostic,
    ViewerReportSeverity, suggest_view_for_each_entity,
};
use re_viewport_blueprint::ViewProperty;

use crate::audio_asset_cache::AudioLoadState;
use crate::visualizer::{AudioAssetVisualizer, AudioEntry};
use crate::waveform::{WaveformViewRange, waveform_ui};

/// The waveform prefers the top of this range and shrinks when the view is smaller.
const WAVEFORM_HEIGHT_RANGE: Rangef = Rangef {
    min: 24.0,
    max: 96.0,
};

#[derive(Default)]
pub struct AudioViewState {
    /// Per-entity zoom of the waveform. Not persisted in the blueprint.
    view_ranges: IntMap<EntityPath, WaveformViewRange>,

    time_advance: TimeAdvance,
}

/// Tracks whether the time cursor moved since the previous frame.
///
/// Compared per frame rather than per pass, because every pass of a frame sees the same time.
#[derive(Default)]
struct TimeAdvance {
    frame_nr: u64,
    time_this_frame: Option<TimeReal>,
    time_last_frame: Option<TimeReal>,
}

impl TimeAdvance {
    /// Whether the time is standing still, e.g. while the user holds or drags the time cursor
    /// during playback.
    ///
    /// Playback is silenced then: the mixer would otherwise resync to the unmoving playhead
    /// over and over, looping a short slice of audio.
    fn is_held(&mut self, frame_nr: u64, time: Option<TimeReal>) -> bool {
        if frame_nr != self.frame_nr {
            self.frame_nr = frame_nr;
            self.time_last_frame = self.time_this_frame;
            self.time_this_frame = time;
        }
        time.is_some() && self.time_last_frame == time
    }
}

impl ViewState for AudioViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn heap_size_bytes(&self) -> u64 {
        let Self {
            view_ranges,
            time_advance: _,
        } = self;
        re_byte_size::SizeBytes::heap_size_bytes(view_ranges)
    }
}

/// Where the playhead is relative to one audio asset, given the current time.
struct Playhead {
    /// Seconds since the asset started, possibly negative or past its end.
    offset_secs: f64,

    /// Playback speed multiplier, where 1.0 is real time.
    speed: f32,

    playing: bool,
}

/// The audio view class.
#[derive(Default)]
pub struct AudioView;

type ViewType = re_sdk_types::blueprint::views::AudioView;

impl ViewClass for AudioView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Audio"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_AUDIO
    }

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        let egui::InputOptions {
            zoom_modifier,
            horizontal_scroll_modifier,
            ..
        } = egui::InputOptions::default(); // This is OK, since we don't allow the user to change these modifiers.

        Help::new("Audio view")
            .docs_link("https://rerun.io/docs/reference/types/views/audio_view")
            .markdown(
                "Shows the waveform of an audio asset and plays it while time is playing on a \
                 temporal timeline, starting from the time the asset was logged.",
            )
            .control("Pan", (icons::LEFT_MOUSE_CLICK, "+", "drag"))
            .control(
                "Horizontal pan",
                IconText::from_modifiers_and(os, horizontal_scroll_modifier, icons::SCROLL),
            )
            .control(
                "Zoom",
                IconText::from_modifiers_and(os, zoom_modifier, icons::SCROLL),
            )
            .control("Move time cursor", icons::LEFT_MOUSE_CLICK)
            .control(
                "Drag time cursor",
                (icons::LEFT_MOUSE_CLICK, "+", "drag cursor"),
            )
            .control("Reset view", ("double", icons::LEFT_MOUSE_CLICK))
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<AudioAssetVisualizer>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<AudioViewState>::default()
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        suggest_view_for_each_entity::<AudioAssetVisualizer>(ctx, include_entity)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<re_viewer_context::ViewClassUiOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let tokens = ui.tokens();
        let entries = system_output
            .visualizer_data_or_default::<Vec<AudioEntry>>(AudioAssetVisualizer::identifier())?;

        let volume = {
            let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);
            let playback = ViewProperty::from_archetype::<AudioPlayback>(&view_ctx);
            playback.component_or_fallback::<Volume>(
                &view_ctx,
                AudioPlayback::descriptor_volume().component,
            )?
        };
        let state = state.downcast_mut::<AudioViewState>()?;

        let time_ctrl = ctx.time_ctrl;
        let is_temporal = is_temporal(time_ctrl);
        let time_is_held = state
            .time_advance
            .is_held(ui.ctx().cumulative_frame_nr(), time_ctrl.time());
        let reports = view_reports(
            &entries,
            is_temporal,
            ctx.audio_output()
                .and_then(re_viewer_context::AudioOutput::output_error),
        );

        let frame = egui::Frame::new().inner_margin(tokens.view_padding());
        let response = frame
            .show(ui, |ui| {
                let inner_ui_builder = egui::UiBuilder::new()
                    .layout(egui::Layout::top_down(egui::Align::LEFT))
                    .sense(Sense::click());
                ui.scope_builder(inner_ui_builder, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if entries.is_empty() {
                                ui.weak("(empty)");
                            }
                            // The selection panel already names a single entity.
                            let show_entity_path = 1 < entries.len();
                            let settings = EntryUiSettings {
                                timeline: query.timeline,
                                is_temporal,
                                volume: *volume.0,
                                waveform_height: waveform_height(
                                    ui,
                                    entries.len(),
                                    show_entity_path,
                                ),
                                show_entity_path,
                            };
                            for (index, entry) in entries.iter().enumerate() {
                                let playhead = playhead_for(entry, time_ctrl, time_is_held);
                                let view_range = state
                                    .view_ranges
                                    .entry(entry.entity_path.clone())
                                    .or_default();
                                audio_entry_ui(
                                    ctx,
                                    ui,
                                    query.view_id,
                                    entry,
                                    playhead.as_ref(),
                                    view_range,
                                    &settings,
                                );
                                if index + 1 < entries.len() {
                                    ui.add_space(ui.spacing().item_spacing.y);
                                }
                            }
                        });
                    ui.response()
                })
                .inner
            })
            .inner;

        let hovered = ui.ctx().rect_contains_pointer(ui.layer_id(), response.rect);
        if hovered {
            ctx.selection_state().set_hovered(Item::View(query.view_id));
        }
        if response.clicked() {
            ctx.command_sender()
                .send_system(SystemCommand::set_selection(Item::View(query.view_id)));
        }

        Ok(re_viewer_context::ViewClassUiOutput { reports })
    }
}

/// Problems that keep the whole view from playing, for the view's error list.
///
/// Problems with a single entity are reported by [`AudioAssetVisualizer`] instead.
fn view_reports(
    entries: &[AudioEntry],
    is_temporal: bool,
    output_error: Option<OutputError>,
) -> Vec<ViewerDiagnostic> {
    let mut reports = Vec::new();

    if !entries.is_empty() && !is_temporal {
        reports.push(ViewerDiagnostic {
            severity: ViewerReportSeverity::Warning,
            summary: "Audio only plays on duration or timestamp timelines".to_owned(),
            details: None,
        });
    }

    if !entries.is_empty()
        && let Some(err) = output_error
    {
        reports.push(ViewerDiagnostic {
            severity: ViewerReportSeverity::Warning,
            summary: "No audio output device available".to_owned(),
            details: Some(err.to_string()),
        });
    }

    reports
}

/// `None` on a sequence timeline, where the asset cannot be placed in time.
fn playhead_for(
    entry: &AudioEntry,
    time_ctrl: &TimeControl,
    time_is_held: bool,
) -> Option<Playhead> {
    if !is_temporal(time_ctrl) {
        return None;
    }
    let now = time_ctrl.time()?;
    Some(Playhead {
        offset_secs: 1e-9 * (now - entry.data_time).as_f64(),
        speed: time_ctrl.speed(),
        playing: !time_is_held
            && matches!(
                time_ctrl.play_state(),
                PlayState::Playing | PlayState::Following
            ),
    })
}

fn is_temporal(time_ctrl: &TimeControl) -> bool {
    time_ctrl.time_type().is_some_and(TimeType::is_temporal)
}

/// Shrinks the waveform so all entries fit the view, down to a minimum.
fn waveform_height(ui: &egui::Ui, num_entries: usize, show_entity_path: bool) -> f32 {
    let num_text_rows = if show_entity_path { 2.0 } else { 1.0 };
    let per_entry_overhead =
        num_text_rows * ui.spacing().interact_size.y + 4.0 * ui.spacing().item_spacing.y;
    let available = ui.available_height() / num_entries.max(1) as f32 - per_entry_overhead;
    WAVEFORM_HEIGHT_RANGE.clamp(available)
}

/// Per-frame settings shared by every entry in the view.
struct EntryUiSettings {
    timeline: TimelineName,
    is_temporal: bool,
    volume: f32,
    waveform_height: f32,
    show_entity_path: bool,
}

fn audio_entry_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: ViewId,
    entry: &AudioEntry,
    playhead: Option<&Playhead>,
    view_range: &mut WaveformViewRange,
    settings: &EntryUiSettings,
) {
    let EntryUiSettings {
        timeline,
        is_temporal,
        volume,
        waveform_height,
        show_entity_path,
    } = *settings;
    if show_entity_path {
        use re_ui::SyntaxHighlighting as _;
        ui.label(entry.entity_path.syntax_highlighted(ui.style()));
    }

    match &entry.state {
        AudioLoadState::Loading => {
            ui.loading_indicator("Decoding audio");
            ui.ctx().request_repaint();
        }
        AudioLoadState::Failed(_) => {}
        AudioLoadState::Ready(decoded) => {
            let buffer = &decoded.buffer;
            let duration_secs = buffer.duration_secs();

            let output = waveform_ui(
                ui,
                decoded,
                playhead.map(|p| p.offset_secs),
                view_range,
                waveform_height,
            );
            if let Some(seek_to_secs) = output.seek_to_secs
                && playhead.is_some()
            {
                let target = TimeReal::from(entry.data_time) + TimeReal::from(seek_to_secs * 1e9);
                ctx.send_time_commands([TimeControlCommand::SetTime(target)]);
            }

            info_line_ui(ui, entry, buffer, playhead);

            // The clip plays on past the single time it was logged at, so tell the time
            // control how far the timeline really reaches.
            if is_temporal {
                let duration_ns = (duration_secs * 1e9).round() as i64;
                ctx.send_time_commands([TimeControlCommand::ExtendTimeRange {
                    timeline,
                    range: AbsoluteTimeRange::new(
                        entry.data_time,
                        entry.data_time.saturating_add(duration_ns),
                    ),
                }]);
            }

            // Keyed by view and entity only, so a new blob on the same entity replaces the
            // running stream instead of overlapping with it.
            let stream_id = StreamId(Hash64::hash((view_id, &entry.entity_path)).hash64());
            let audible = playhead.is_some_and(|playhead| {
                playhead.playing
                    && 0.0 <= playhead.offset_secs
                    && playhead.offset_secs < duration_secs
            });
            if let Some(playhead) = playhead
                && audible
                && let Some(audio_output) = ctx.audio_output()
            {
                audio_output.request(StreamRequest {
                    id: stream_id,
                    buffer: buffer.clone(),
                    position_secs: playhead.offset_secs,
                    speed: playhead.speed,
                    volume,
                });
            }
        }
    }
}

/// `max_decimals` must be one of the values [`re_format::DurationFormatOptions`] accepts: 0, 1, 3, 6, or 9.
fn format_secs(secs: f64, max_decimals: usize) -> String {
    re_log::debug_assert!(
        [0, 1, 3, 6, 9].contains(&max_decimals),
        "unsupported max_decimals: {max_decimals}"
    );
    re_format::DurationFormatOptions::default()
        .with_only_seconds(false)
        .with_max_decimals(max_decimals)
        .format_nanos((secs * 1e9).round() as i64)
}

fn info_line_ui(
    ui: &mut egui::Ui,
    entry: &AudioEntry,
    buffer: &Arc<AudioBuffer>,
    playhead: Option<&Playhead>,
) {
    let mut parts = vec![
        format_secs(buffer.duration_secs(), 1),
        format!("{} Hz", re_format::format_uint(buffer.sample_rate)),
        match buffer.num_channels {
            1 => "mono".to_owned(),
            2 => "stereo".to_owned(),
            n => re_format::format_plural_s(n, "channel"),
        },
    ];
    if let Some(media_type) = &entry.media_type {
        parts.push(media_type.to_string());
    }
    if let Some(playhead) = playhead {
        parts.push(format!("at {}", format_secs(playhead.offset_secs, 3)));
    }

    ui.horizontal_wrapped(|ui| {
        ui.weak(parts.join(" · "));
    });
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| AudioView.help(ctx));
}

#[cfg(test)]
mod tests {
    use re_log_types::TimeInt;

    use super::*;

    fn entry(path: &str, data_time: TimeInt, state: AudioLoadState) -> AudioEntry {
        AudioEntry {
            entity_path: EntityPath::from(path),
            data_time,
            media_type: None,
            state,
        }
    }

    fn summaries(reports: &[ViewerDiagnostic]) -> Vec<(ViewerReportSeverity, &str)> {
        reports
            .iter()
            .map(|report| (report.severity, report.summary.as_str()))
            .collect()
    }

    #[test]
    fn view_reports_cover_every_view_level_problem() {
        let entries = [entry(
            "loading",
            TimeInt::new_temporal(0),
            AudioLoadState::Loading,
        )];

        let reports = view_reports(
            &entries,
            false,
            Some(OutputError::Device("no device".to_owned())),
        );

        assert_eq!(
            summaries(&reports),
            [
                (
                    ViewerReportSeverity::Warning,
                    "Audio only plays on duration or timestamp timelines"
                ),
                (
                    ViewerReportSeverity::Warning,
                    "No audio output device available"
                ),
            ]
        );
    }

    #[test]
    fn view_reports_are_empty_without_entries() {
        let reports = view_reports(
            &[],
            false,
            Some(OutputError::Device("no device".to_owned())),
        );
        assert_eq!(reports, [], "an empty view has nothing to report");
    }
}
