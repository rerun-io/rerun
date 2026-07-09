use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

use egui_plot::{Line, Plot, PlotPoints, VLine};
use re_log_types::{EntityPath, TimeInt, TimeReal};
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, UiExt as _};
use re_viewer_context::{
    IdentifiedViewSystem as _, IndicatedEntities, PerVisualizerType, QueryRange,
    RecommendedVisualizers, SystemExecutionOutput, TimeControlCommand, ViewClass,
    ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery, ViewState,
    ViewStateExt as _, ViewSystemExecutionError, ViewSystemIdentifier, ViewerContext,
    VisualizableReason, suggest_view_for_each_entity,
};

use crate::processing::{AudioProcessingSettings, FilterKind, WindowFunction};
use crate::visualizer_system::{
    AudioAnnotationSpan, AudioAnnotationSystem, AudioVisualizerSystem, AudioWaveform,
};

#[derive(Default)]
pub struct AudioView;

type ViewType = re_sdk_types::blueprint::views::AudioView;

#[derive(Default, re_byte_size::SizeBytes)]
pub struct AudioViewState {
    channel_visible: Vec<bool>,
    show_mixdown: bool,
    processing: AudioProcessingSettings,
    #[cfg(not(target_arch = "wasm32"))]
    #[size_bytes(ignore)]
    playback: Mutex<Option<crate::playback::AudioPlayback>>,
    #[cfg(not(target_arch = "wasm32"))]
    playback_error: Option<String>,
}

impl ViewState for AudioViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn heap_size_bytes(&self) -> u64 {
        re_byte_size::SizeBytes::heap_size_bytes(self)
    }
}

impl ViewClass for AudioView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Audio"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_TIMESERIES
    }

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        let egui::InputOptions {
            zoom_modifier,
            horizontal_scroll_modifier,
            ..
        } = egui::InputOptions::default();

        Help::new("Audio view")
            .docs_link("https://rerun.io/docs/reference/types/views/audio_view")
            .markdown("An audio waveform view for PCM audio clips logged on a time timeline.")
            .control("Pan", (re_ui::icons::LEFT_MOUSE_CLICK, "+", "drag"))
            .control(
                "Horizontal pan",
                re_ui::IconText::from_modifiers_and(
                    os,
                    horizontal_scroll_modifier,
                    re_ui::icons::SCROLL,
                ),
            )
            .control(
                "Zoom",
                re_ui::IconText::from_modifiers_and(os, zoom_modifier, re_ui::icons::SCROLL),
            )
            .control("Scrub time", (re_ui::icons::LEFT_MOUSE_CLICK, "click/drag"))
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<AudioVisualizerSystem>()?;
        system_registry.register_visualizer::<AudioAnnotationSystem>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<AudioViewState>::default()
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        ViewClassLayoutPriority::Low
    }

    fn supports_visible_time_range(&self) -> bool {
        true
    }

    fn default_query_range(&self, _view_state: &dyn ViewState) -> QueryRange {
        QueryRange::TimeRange(re_sdk_types::datatypes::TimeRange::EVERYTHING)
    }

    fn recommended_visualizers_for_entity(
        &self,
        _entity_path: &EntityPath,
        visualizers_with_reason: &[(ViewSystemIdentifier, &VisualizableReason)],
        _indicated_entities_per_visualizer: &PerVisualizerType<&IndicatedEntities>,
    ) -> RecommendedVisualizers {
        if visualizers_with_reason
            .iter()
            .any(|(viz, _)| *viz == AudioVisualizerSystem::identifier())
        {
            RecommendedVisualizers::default(AudioVisualizerSystem::identifier())
        } else {
            RecommendedVisualizers::empty()
        }
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        suggest_view_for_each_entity::<AudioVisualizerSystem>(ctx, include_entity)
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<AudioViewState>()?;
        let waveforms = system_output
            .visualizer_data_or_default::<BTreeMap<EntityPath, AudioWaveform>>(
                AudioVisualizerSystem::identifier(),
            )?;
        let annotations = system_output.visualizer_data_or_default::<Vec<AudioAnnotationSpan>>(
            AudioAnnotationSystem::identifier(),
        )?;

        if waveforms.is_empty() && annotations.is_empty() {
            ui.centered_and_justified(|ui| ui.label("(empty)"));
            return Ok(());
        }

        let max_channels = waveforms
            .values()
            .map(AudioWaveform::num_channels)
            .max()
            .unwrap_or_default();
        resize_channel_visibility(&mut state.channel_visible, max_channels);

        playback_progress_ui(ctx, ui, state);
        toolbar_ui(ctx, ui, state, max_channels, waveforms.values().next());
        plot_audio(ctx, ui, state, query.view_id, &waveforms, &annotations);

        Ok(())
    }
}

fn resize_channel_visibility(channel_visible: &mut Vec<bool>, num_channels: usize) {
    if channel_visible.len() < num_channels {
        channel_visible.resize(num_channels, true);
    } else if channel_visible.len() > num_channels {
        channel_visible.truncate(num_channels);
    }
}

fn toolbar_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut AudioViewState,
    num_channels: usize,
    first_waveform: Option<&AudioWaveform>,
) {
    ui.horizontal_wrapped(|ui| {
        playback_buttons_ui(ctx, ui, state, first_waveform);
        ui.checkbox(&mut state.show_mixdown, "mix");
        for channel_idx in 0..num_channels {
            let label = format!("ch {}", channel_idx + 1);
            ui.checkbox(&mut state.channel_visible[channel_idx], label);
        }
    });

    processing_ui(ui, &mut state.processing);

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(err) = &state.playback_error {
        ui.error_label(err);
    }
}

fn processing_ui(ui: &mut egui::Ui, processing: &mut AudioProcessingSettings) {
    ui.horizontal_wrapped(|ui| {
        ui.label("window");
        egui::ComboBox::from_id_salt("audio_window")
            .selected_text(processing.window.label())
            .show_ui(ui, |ui| {
                for window in WindowFunction::ALL {
                    ui.selectable_value(&mut processing.window, window, window.label());
                }
            });

        ui.label("filter");
        egui::ComboBox::from_id_salt("audio_filter")
            .selected_text(processing.filter.label())
            .show_ui(ui, |ui| {
                for filter in FilterKind::ALL {
                    ui.selectable_value(&mut processing.filter, filter, filter.label());
                }
            });

        if matches!(
            processing.filter,
            FilterKind::HighPass | FilterKind::BandPass
        ) {
            ui.label("low Hz");
            ui.add(
                egui::DragValue::new(&mut processing.low_cut_hz)
                    .range(1.0..=96_000.0)
                    .speed(10.0),
            );
        }

        if matches!(
            processing.filter,
            FilterKind::LowPass | FilterKind::BandPass
        ) {
            ui.label("high Hz");
            ui.add(
                egui::DragValue::new(&mut processing.high_cut_hz)
                    .range(1.0..=96_000.0)
                    .speed(10.0),
            );
        }

        if processing.is_active() && ui.button("Reset").clicked() {
            *processing = AudioProcessingSettings::default();
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn playback_buttons_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut AudioViewState,
    first_waveform: Option<&AudioWaveform>,
) {
    let is_playing = state
        .playback
        .lock()
        .is_ok_and(|playback| playback.is_some());
    if ui
        .add_enabled(
            !is_playing && first_waveform.is_some(),
            egui::Button::new("Play"),
        )
        .clicked()
        && let Some(waveform) = first_waveform
    {
        let enabled_channels = enabled_channels(state);
        let cursor_time = ctx.time_ctrl.time_int().unwrap_or(TimeInt::ZERO);
        match crate::playback::AudioPlayback::start(
            waveform,
            &enabled_channels,
            state.show_mixdown,
            &state.processing,
            cursor_time,
        ) {
            Ok(playback) => {
                if let Ok(mut current_playback) = state.playback.lock() {
                    *current_playback = Some(playback);
                }
                state.playback_error = None;
                ctx.send_time_commands([TimeControlCommand::Pause]);
            }
            Err(err) => {
                if let Ok(mut current_playback) = state.playback.lock() {
                    *current_playback = None;
                }
                state.playback_error = Some(err);
            }
        }
    }

    if ui
        .add_enabled(is_playing, egui::Button::new("Stop"))
        .clicked()
    {
        if let Ok(mut current_playback) = state.playback.lock() {
            *current_playback = None;
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn playback_buttons_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _state: &mut AudioViewState,
    _first_waveform: Option<&AudioWaveform>,
) {
    ui.add_enabled(false, egui::Button::new("Play"));
}

#[cfg(not(target_arch = "wasm32"))]
fn playback_progress_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, state: &mut AudioViewState) {
    let Ok(mut playback) = state.playback.lock() else {
        return;
    };

    if let Some(current_playback) = playback.as_ref() {
        if current_playback.is_finished() {
            let time = current_playback.current_time();
            *playback = None;
            ctx.send_time_commands([TimeControlCommand::SetTimeClamped(time)]);
        } else {
            ctx.send_time_commands([TimeControlCommand::SetTimeClamped(
                current_playback.current_time(),
            )]);
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn playback_progress_ui(_ctx: &ViewerContext<'_>, _ui: &mut egui::Ui, _state: &mut AudioViewState) {
}

#[cfg(not(target_arch = "wasm32"))]
fn enabled_channels(state: &AudioViewState) -> Vec<usize> {
    state
        .channel_visible
        .iter()
        .enumerate()
        .filter_map(|(idx, visible)| visible.then_some(idx))
        .collect()
}

fn plot_audio(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &AudioViewState,
    view_id: ViewId,
    waveforms: &BTreeMap<EntityPath, AudioWaveform>,
    annotations: &[AudioAnnotationSpan],
) {
    let current_time = ctx.time_ctrl.time_int().unwrap_or(TimeInt::ZERO);
    let current_time_ns = current_time.as_f64();
    let cursor_color = ui.visuals().selection.stroke.color;

    let mut x_range = None::<(f64, f64)>;
    for waveform in waveforms.values() {
        if let Some((min, max)) = waveform.time_range_ns() {
            x_range = Some(match x_range {
                Some((range_min, range_max)) => (range_min.min(min), range_max.max(max)),
                None => (min, max),
            });
        }
    }
    for annotation in annotations {
        let min = annotation.start_time.as_f64();
        let max = annotation.end_time.as_f64();
        x_range = Some(match x_range {
            Some((range_min, range_max)) => (range_min.min(min), range_max.max(max)),
            None => (min, max),
        });
    }

    let y_max = waveforms
        .values()
        .map(AudioWaveform::num_channels)
        .max()
        .unwrap_or(1) as f64;

    let plot = Plot::new("audio_waveform_plot")
        .allow_boxed_zoom(false)
        .allow_drag(true)
        .allow_scroll(true)
        .show_x(false)
        .show_y(false)
        .clamp_grid(true)
        .include_y(-1.25)
        .include_y(y_max);

    let response = plot.show(ui, |plot_ui| {
        if let Some((min, max)) = x_range {
            plot_ui.set_plot_bounds_x(min..=max.max(min + 1.0));
        }

        plot_ui.vline(
            VLine::new("time cursor", current_time_ns)
                .color(cursor_color)
                .width(1.5),
        );

        for (entity_idx, (entity_path, waveform)) in waveforms.iter().enumerate() {
            draw_waveform(plot_ui, state, entity_idx, entity_path, waveform);
        }
        draw_annotations(plot_ui, annotations, y_max);

        if (plot_ui.response().clicked() || plot_ui.response().dragged())
            && let Some(pointer) = plot_ui.pointer_coordinate()
        {
            ctx.send_time_commands([
                TimeControlCommand::SetTimeClamped(TimeReal::from(pointer.x)),
                TimeControlCommand::Pause,
            ]);
        }
    });

    if response.response.hovered() {
        ctx.selection_state()
            .set_hovered(re_viewer_context::Item::View(view_id));
    }
}

fn draw_annotations(
    plot_ui: &mut egui_plot::PlotUi<'_>,
    annotations: &[AudioAnnotationSpan],
    y_max: f64,
) {
    for annotation in annotations {
        let start = annotation.start_time.as_f64();
        let end = annotation.end_time.as_f64().max(start);
        let color = annotation
            .color
            .map(egui::Color32::from)
            .unwrap_or_else(|| egui::Color32::from_rgba_unmultiplied(255, 210, 80, 70));
        let stroke = egui::Stroke::new(1.0, color.gamma_multiply(1.8));

        plot_ui.polygon(
            egui_plot::Polygon::new(
                annotation.text.clone(),
                vec![
                    [start, -0.65],
                    [end, -0.65],
                    [end, y_max + 0.35],
                    [start, y_max + 0.35],
                ],
            )
            .fill_color(color)
            .stroke(stroke),
        );
        plot_ui.text(
            egui_plot::Text::new(
                annotation.text.clone(),
                egui_plot::PlotPoint::new((start + end) * 0.5, y_max + 0.45),
                annotation.text.clone(),
            )
            .color(stroke.color),
        );
    }
}

fn draw_waveform(
    plot_ui: &mut egui_plot::PlotUi<'_>,
    state: &AudioViewState,
    entity_idx: usize,
    entity_path: &EntityPath,
    waveform: &AudioWaveform,
) {
    let channel_count = waveform.num_channels();
    for channel_idx in 0..channel_count {
        if !state
            .channel_visible
            .get(channel_idx)
            .copied()
            .unwrap_or(true)
        {
            continue;
        }

        let points = channel_points(waveform, channel_idx, channel_idx as f64, &state.processing);
        if points.is_empty() {
            continue;
        }

        let name = waveform
            .channel_names
            .get(channel_idx)
            .cloned()
            .unwrap_or_else(|| format!("{entity_path} ch {}", channel_idx + 1));
        plot_ui.line(
            Line::new(name, PlotPoints::Owned(points))
                .width(1.25)
                .color(channel_color(entity_idx, channel_idx)),
        );
    }

    if state.show_mixdown && channel_count > 1 {
        let points = mixdown_points(
            waveform,
            channel_count,
            channel_count as f64,
            &state.processing,
        );
        if !points.is_empty() {
            plot_ui.line(
                Line::new(format!("{entity_path} mix"), PlotPoints::Owned(points))
                    .width(1.5)
                    .color(egui::Color32::WHITE),
            );
        }
    }
}

fn channel_points(
    waveform: &AudioWaveform,
    channel_idx: usize,
    y_offset: f64,
    processing: &AudioProcessingSettings,
) -> Vec<egui_plot::PlotPoint> {
    let mut points = Vec::new();
    for chunk in &waveform.chunks {
        let Some(samples) = chunk.channels.get(channel_idx) else {
            continue;
        };
        let processed = crate::processing::process_samples(samples, chunk.sample_rate, processing);
        points.extend(processed.iter().enumerate().map(|(sample_idx, sample)| {
            egui_plot::PlotPoint::new(
                chunk.start_time.as_f64() + sample_idx as f64 / chunk.sample_rate * 1_000_000_000.0,
                y_offset + sample.clamp(-1.0, 1.0) * 0.45,
            )
        }));
    }
    points
}

fn mixdown_points(
    waveform: &AudioWaveform,
    channel_count: usize,
    y_offset: f64,
    processing: &AudioProcessingSettings,
) -> Vec<egui_plot::PlotPoint> {
    let mut points = Vec::new();
    for chunk in &waveform.chunks {
        let sample_count = chunk
            .channels
            .iter()
            .map(Vec::len)
            .min()
            .unwrap_or_default();
        let mut mixed = Vec::with_capacity(sample_count);
        for sample_idx in 0..sample_count {
            let sum = chunk
                .channels
                .iter()
                .take(channel_count)
                .map(|channel| channel[sample_idx])
                .sum::<f64>();
            mixed.push(sum / channel_count as f64);
        }

        let processed = crate::processing::process_samples(&mixed, chunk.sample_rate, processing);
        for (sample_idx, sample) in processed.iter().enumerate() {
            points.push(egui_plot::PlotPoint::new(
                chunk.start_time.as_f64() + sample_idx as f64 / chunk.sample_rate * 1_000_000_000.0,
                y_offset + sample.clamp(-1.0, 1.0) * 0.45,
            ));
        }
    }
    points
}

fn channel_color(entity_idx: usize, channel_idx: usize) -> egui::Color32 {
    const COLORS: [egui::Color32; 8] = [
        egui::Color32::from_rgb(80, 170, 255),
        egui::Color32::from_rgb(255, 170, 75),
        egui::Color32::from_rgb(100, 210, 140),
        egui::Color32::from_rgb(235, 110, 150),
        egui::Color32::from_rgb(175, 135, 255),
        egui::Color32::from_rgb(120, 220, 220),
        egui::Color32::from_rgb(230, 220, 120),
        egui::Color32::from_rgb(200, 200, 200),
    ];
    COLORS[(entity_idx + channel_idx) % COLORS.len()]
}
