use egui_plot::{Legend, Line, Plot, Points};

use re_arrow_store::TimeType;
use re_data_ui::visible_history::visible_history_ui;
use re_format::next_grid_tick_magnitude_ns;
use re_log_types::{EntityPath, TimeZone};
use re_space_view::controls;
use re_viewer_context::external::re_data_store::{
    EntityPropertyMap, ExtraQueryHistory, VisibleHistory,
};
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassName, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewPartCollection, ViewQuery,
    ViewerContext,
};

use crate::view_part_system::{PlotSeriesKind, TimeSeriesSystem};

#[derive(Clone)]
pub struct TimeSeriesSpaceViewState {
    visible_history: ExtraQueryHistory,
}

impl Default for TimeSeriesSpaceViewState {
    fn default() -> Self {
        Self {
            visible_history: ExtraQueryHistory {
                enabled: false,

                // this mirrors TimeSeriesSystem's behaviors when `enabled` is `false`
                nanos: VisibleHistory::ALL,
                sequences: VisibleHistory::ALL,
            },
        }
    }
}

impl SpaceViewState for TimeSeriesSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl TimeSeriesSpaceViewState {
    fn selection_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _space_view_id: SpaceViewId,
    ) {
        visible_history_ui(ctx, ui, None, &mut self.visible_history);
    }
}

#[derive(Default)]
pub struct TimeSeriesSpaceView;

impl SpaceViewClass for TimeSeriesSpaceView {
    type State = TimeSeriesSpaceViewState;

    fn name(&self) -> SpaceViewClassName {
        "Time Series".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_CHART
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

        layout.add("Pan by dragging, or scroll (+ ");
        layout.add(controls::HORIZONTAL_SCROLL_MODIFIER);
        layout.add(" for horizontal).\n");

        layout.add("Zoom with pinch gesture or scroll + ");
        layout.add(controls::ZOOM_SCROLL_MODIFIER);
        layout.add(".\n");

        layout.add("Scroll + ");
        layout.add(controls::ASPECT_SCROLL_MODIFIER);
        layout.add(" to change the aspect ratio.\n");

        layout.add("Drag ");
        layout.add(controls::SELECTION_RECT_ZOOM_BUTTON);
        layout.add(" to zoom in/out using a selection.\n");

        layout.add("Click ");
        layout.add(controls::MOVE_TIME_CURSOR_BUTTON);
        layout.add(" to move the time cursor.\n\n");

        layout.add_button_text(controls::RESET_VIEW_BUTTON_TEXT);
        layout.add(" to reset the view.");

        layout.layout_job.into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_part_system::<TimeSeriesSystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn on_frame_start(
        &self,
        _ctx: &mut ViewerContext<'_>,
        state: &Self::State,
        entity_paths: &re_viewer_context::PerSystemEntities,
        entity_properties: &mut EntityPropertyMap,
    ) {
        // synchronise all entities with the Space View's visible history state.
        for entity_path in entity_paths.values().flat_map(|paths| paths.iter()) {
            let mut props = entity_properties.get(entity_path);
            props.visible_history = state.visible_history;
            entity_properties.set(entity_path.clone(), props);
        }
    }

    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        state.selection_ui(ctx, ui, space_view_id);
    }

    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        _view_ctx: &ViewContextCollection,
        parts: &ViewPartCollection,
        _query: &ViewQuery<'_>,
        _draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let time_ctrl = &ctx.rec_cfg.time_ctrl;
        let current_time = time_ctrl.time_i64();
        let time_type = time_ctrl.time_type();
        let timeline = time_ctrl.timeline();

        let timeline_name = timeline.name().to_string();

        let time_series = parts.get::<TimeSeriesSystem>()?;

        // Compute the minimum time/X value for the entire plot…
        let min_time = time_series
            .lines
            .iter()
            .flat_map(|line| line.points.iter().map(|p| p.0))
            .min()
            .unwrap_or(0);

        // …then use that as an offset to avoid nasty precision issues with
        // large times (nanos since epoch does not fit into an f64).
        let time_offset = if timeline.typ() == TimeType::Time {
            // In order to make the tick-marks on the time axis fall on whole days, hours, minutes etc,
            // we need to round to a whole day:
            round_ns_to_start_of_day(min_time)
        } else {
            min_time
        };

        // use timeline_name as part of id, so that egui stores different pan/zoom for different timelines
        let plot_id_src = ("plot", &timeline_name);

        let zoom_both_axis = !ui.input(|i| i.modifiers.contains(controls::ASPECT_SCROLL_MODIFIER));

        let time_zone_for_timestamps = ctx.app_options.time_zone_for_timestamps;
        let mut plot = Plot::new(plot_id_src)
            .allow_zoom(egui_plot::AxisBools {
                x: true,
                y: zoom_both_axis,
            })
            .legend(Legend {
                position: egui_plot::Corner::RightBottom,
                ..Default::default()
            })
            .x_axis_formatter(move |time, _, _| {
                format_time(
                    time_type,
                    time as i64 + time_offset,
                    time_zone_for_timestamps,
                )
            })
            .label_formatter(move |name, value| {
                let name = if name.is_empty() { "y" } else { name };
                let is_integer = value.y.round() == value.y;
                let decimals = if is_integer { 0 } else { 5 };
                format!(
                    "{timeline_name}: {}\n{name}: {:.*}",
                    time_type.format(
                        (value.x as i64 + time_offset).into(),
                        time_zone_for_timestamps
                    ),
                    decimals,
                    value.y,
                )
            });

        if timeline.typ() == TimeType::Time {
            let canvas_size = ui.available_size();
            plot = plot.x_grid_spacer(move |spacer| ns_grid_spacer(canvas_size, &spacer));
        }

        let egui_plot::PlotResponse {
            inner: time_x,
            response,
            transform,
        } = plot.show(ui, |plot_ui| {
            if plot_ui.response().secondary_clicked() {
                let timeline = ctx.rec_cfg.time_ctrl.timeline();
                ctx.rec_cfg.time_ctrl.set_timeline_and_time(
                    *timeline,
                    plot_ui.pointer_coordinate().unwrap().x as i64 + time_offset,
                );
                ctx.rec_cfg.time_ctrl.pause();
            }

            for line in &time_series.lines {
                let points = line
                    .points
                    .iter()
                    .map(|p| [(p.0 - time_offset) as _, p.1])
                    .collect::<Vec<_>>();

                let color = line.color;

                match line.kind {
                    PlotSeriesKind::Continuous => plot_ui.line(
                        Line::new(points)
                            .name(&line.label)
                            .color(color)
                            .width(line.width),
                    ),
                    PlotSeriesKind::Scatter => plot_ui.points(
                        Points::new(points)
                            .name(&line.label)
                            .color(color)
                            .radius(line.width),
                    ),
                }
            }

            current_time.map(|current_time| {
                let time_x = (current_time - time_offset) as f64;
                plot_ui.screen_from_plot([time_x, 0.0].into()).x
            })
        });

        if let Some(time_x) = time_x {
            let interact_radius = ui.style().interaction.resize_grab_radius_side;
            let line_rect = egui::Rect::from_x_y_ranges(time_x..=time_x, response.rect.y_range())
                .expand(interact_radius);

            let time_drag_id = ui.id().with("time_drag");
            let response = ui
                .interact(line_rect, time_drag_id, egui::Sense::drag())
                .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);

            if response.dragged() {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let time =
                        time_offset + transform.value_from_position(pointer_pos).x.round() as i64;

                    let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
                    time_ctrl.set_time(time);
                    time_ctrl.pause();
                }
            }

            let stroke = if response.dragged() {
                ui.style().visuals.widgets.active.fg_stroke
            } else if response.hovered() {
                ui.style().visuals.widgets.hovered.fg_stroke
            } else {
                ui.visuals().widgets.inactive.fg_stroke
            };
            ctx.re_ui
                .paint_time_cursor(ui.painter(), time_x, response.rect.y_range(), stroke);
        }
        Ok(())
    }
}

fn format_time(time_type: TimeType, time_int: i64, time_zone_for_timestamps: TimeZone) -> String {
    if time_type == TimeType::Time {
        let time = re_log_types::Time::from_ns_since_epoch(time_int);
        time.format_time_compact(time_zone_for_timestamps)
    } else {
        time_type.format(
            re_log_types::TimeInt::from(time_int),
            time_zone_for_timestamps,
        )
    }
}

fn ns_grid_spacer(
    canvas_size: egui::Vec2,
    input: &egui_plot::GridInput,
) -> Vec<egui_plot::GridMark> {
    let minimum_medium_line_spacing = 150.0; // ≈min size of a label
    let max_medium_lines = canvas_size.x as f64 / minimum_medium_line_spacing;

    let (min_ns, max_ns) = input.bounds;
    let width_ns = max_ns - min_ns;

    let mut small_spacing_ns = 1;
    while width_ns / (next_grid_tick_magnitude_ns(small_spacing_ns) as f64) > max_medium_lines {
        small_spacing_ns = next_grid_tick_magnitude_ns(small_spacing_ns);
    }
    let medium_spacing_ns = next_grid_tick_magnitude_ns(small_spacing_ns);
    let big_spacing_ns = next_grid_tick_magnitude_ns(medium_spacing_ns);

    let mut current_ns = (min_ns.floor() as i64) / small_spacing_ns * small_spacing_ns;
    let mut marks = vec![];

    while current_ns <= max_ns.ceil() as i64 {
        let is_big_line = current_ns % big_spacing_ns == 0;
        let is_medium_line = current_ns % medium_spacing_ns == 0;

        let step_size = if is_big_line {
            big_spacing_ns
        } else if is_medium_line {
            medium_spacing_ns
        } else {
            small_spacing_ns
        };

        marks.push(egui_plot::GridMark {
            value: current_ns as f64,
            step_size: step_size as f64,
        });

        current_ns += small_spacing_ns;
    }

    marks
}

fn round_ns_to_start_of_day(ns: i64) -> i64 {
    let ns_per_day = 24 * 60 * 60 * 1_000_000_000;
    (ns + ns_per_day / 2) / ns_per_day * ns_per_day
}
