use std::ops::RangeInclusive;

use crate::{
    log_db::ObjectTree,
    misc::time_axis::TimeSourceAxis,
    time_axis::{TimeRange, TimeSourceAxes},
    LogDb, TimeControl, TimeView, ViewerContext,
};
use eframe::egui;
use egui::*;
use log_types::*;

/// A panel that shows objects to the left, time on the top.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TimePanel {
    /// Width of the object name columns previous frame.
    prev_col_width: f32,

    /// The right side of the object name column; updated during its painting.
    #[serde(skip)]
    next_col_right: f32,

    /// The time axis view, regenerated each frame.
    #[serde(skip)]
    time_ranges_ui: TimeRangesUi,
}

impl Default for TimePanel {
    fn default() -> Self {
        Self {
            prev_col_width: 400.0,
            next_col_right: 0.0,
            time_ranges_ui: Default::default(),
        }
    }
}

impl TimePanel {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        crate::profile_function!();

        // play control and current time
        top_row_ui(log_db, context, ui);

        self.next_col_right = ui.min_rect().left(); // this will expand during the call

        // Where the time will be shown.
        let time_x_range = {
            let left = ui.min_rect().left() + self.prev_col_width;
            let right = ui.max_rect().right() - ui.spacing().scroll_bar_width - 8.0;
            left..=right
        };

        self.initialize_time_ranges_ui(log_db, context, time_x_range.clone());

        // includes the time selection and time ticks rows.
        let time_area = Rect::from_x_y_ranges(
            time_x_range.clone(),
            ui.min_rect().bottom()..=ui.max_rect().bottom(),
        );

        let time_line_rect = {
            let response = context
                .time_control
                .time_source_selector(&log_db.time_points, ui);
            self.next_col_right = self.next_col_right.max(response.rect.right());
            let y_range = response.rect.y_range();
            Rect::from_x_y_ranges(time_x_range.clone(), y_range)
        };

        let time_area_painter = ui.painter().with_clip_rect(time_area);

        ui.painter()
            .rect_filled(time_area, 1.0, ui.visuals().extreme_bg_color);

        ui.separator();

        self.paint_time_ranges_and_ticks(ui, &time_area_painter, time_line_rect.y_range());

        let scroll_delta = self.interact_with_time_area(
            &mut context.time_control,
            ui,
            &time_area_painter,
            &time_area,
            &time_line_rect,
        );

        // Don't draw on top of the time ticks
        let lower_time_area_painter = ui.painter().with_clip_rect(Rect::from_x_y_ranges(
            time_x_range,
            ui.min_rect().bottom()..=ui.max_rect().bottom(),
        ));

        // all the object rows:
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                crate::profile_scope!("tree_ui");
                ui.scroll_with_delta(scroll_delta);
                self.tree_ui(log_db, context, &lower_time_area_painter, ui);
            });

        // TODO: fix problem of the fade covering the hlines. Need Shape Z values!
        if true {
            fade_sides(ui, time_area);
        }

        if let Some(time) = context.time_control.time() {
            // so time doesn't get stuck between non-continuos regions
            let time = self.time_ranges_ui.snap_time(time);
            context.time_control.set_time(time);
        }

        // remember where to show the time for next frame:
        let margin = 16.0;
        self.prev_col_width = self.next_col_right - ui.min_rect().left() + margin;
    }

    fn initialize_time_ranges_ui(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        time_x_range: RangeInclusive<f32>,
    ) {
        let time_source_axes = TimeSourceAxes::new(&log_db.time_points);
        if let Some(time_source_axis) = time_source_axes.sources.get(context.time_control.source())
        {
            let time_view = context.time_control.time_view();
            let time_view =
                time_view.unwrap_or_else(|| view_everything(&time_x_range, time_source_axis));

            self.time_ranges_ui =
                TimeRangesUi::new(time_x_range, time_view, &time_source_axis.ranges);
        } else {
            self.time_ranges_ui = Default::default();
        }
    }

    /// Returns a scroll delta
    fn interact_with_time_area(
        &mut self,
        time_control: &mut TimeControl,
        ui: &mut egui::Ui,
        time_area_painter: &egui::Painter,
        full_rect: &Rect,
        time_line_rect: &Rect,
    ) -> Vec2 {
        // time_area: full area.
        // time_line_rect: top part with the second ticks and time marker

        let pointer_pos = ui.input().pointer.hover_pos();
        let is_pointer_in_time_area =
            pointer_pos.map_or(false, |pointer_pos| full_rect.contains(pointer_pos));
        let is_pointer_in_time_line_rect =
            pointer_pos.map_or(false, |pointer_pos| time_line_rect.contains(pointer_pos));

        let response = ui.interact(
            *full_rect,
            ui.id().with("time_area_interact"),
            egui::Sense::click_and_drag(),
        );

        let mut delta_x = 0.0;
        let mut zoom_factor = 1.0;

        let mut parent_scroll_delta = Vec2::ZERO;

        if response.hovered() {
            delta_x += ui.input().scroll_delta.x;
            zoom_factor *= ui.input().zoom_delta_2d().x;
        }

        if response.dragged_by(PointerButton::Primary) {
            delta_x += response.drag_delta().x;
            parent_scroll_delta.y += response.drag_delta().y;
            ui.output().cursor_icon = CursorIcon::AllScroll;
        }
        if response.dragged_by(PointerButton::Secondary) {
            zoom_factor *= (response.drag_delta().y * 0.01).exp();
        }

        if delta_x != 0.0 {
            if let Some(new_view_range) = self.time_ranges_ui.pan(-delta_x) {
                time_control.set_time_view(new_view_range);
            }
        }

        if zoom_factor != 1.0 {
            if let Some(pointer_pos) = pointer_pos {
                if let Some(new_view_range) =
                    self.time_ranges_ui.zoom_at(pointer_pos.x, zoom_factor)
                {
                    time_control.set_time_view(new_view_range);
                }
            }
        }

        if response.double_clicked() {
            time_control.reset_time_view();
        }

        // ------------------------------------------------

        let time_drag_id = ui.id().with("time_drag_id");

        let mut is_hovering = false;
        let mut is_dragging = ui.memory().is_being_dragged(time_drag_id);

        if is_pointer_in_time_line_rect {
            ui.output().cursor_icon = CursorIcon::ResizeHorizontal;
        } else if is_pointer_in_time_area {
            // ui.output().cursor_icon = CursorIcon::AllScroll; // looks ugly
        }

        // show current time as a line:
        if let Some(time) = time_control.time() {
            if let Some(x) = self.time_ranges_ui.x_from_time(time) {
                if let Some(pointer_pos) = pointer_pos {
                    let line_rect = Rect::from_x_y_ranges(x..=x, full_rect.y_range());

                    is_hovering = line_rect.distance_to_pos(pointer_pos)
                        <= ui.style().interaction.resize_grab_radius_side;

                    if ui.input().pointer.any_pressed()
                        && ui.input().pointer.primary_down()
                        && is_hovering
                    {
                        ui.memory().set_dragged_id(time_drag_id);
                        is_dragging = true; // avoid frame delay
                    }
                }

                if is_hovering || is_dragging {
                    ui.output().cursor_icon = CursorIcon::ResizeHorizontal;
                }

                let stroke = if is_dragging {
                    ui.style().visuals.widgets.active.bg_stroke
                } else if is_hovering {
                    ui.style().visuals.widgets.hovered.bg_stroke
                } else {
                    ui.visuals().widgets.inactive.fg_stroke
                };
                let stroke = egui::Stroke {
                    width: 1.5 * stroke.width,
                    ..stroke
                };

                let w = 10.0;
                let triangle = vec![
                    pos2(x - 0.5 * w, full_rect.top()), // left top
                    pos2(x + 0.5 * w, full_rect.top()), // right top
                    pos2(x, full_rect.top() + w),       // bottom
                ];
                time_area_painter.add(egui::Shape::convex_polygon(
                    triangle,
                    stroke.color,
                    egui::Stroke::none(),
                ));
                time_area_painter.vline(x, (full_rect.top() + w)..=full_rect.bottom(), stroke);
            }
        }

        // Show preview: "click here to view time here"
        if let Some(pointer_pos) = pointer_pos {
            if !is_hovering && !is_dragging && is_pointer_in_time_line_rect {
                time_area_painter.vline(
                    pointer_pos.x,
                    full_rect.top()..=ui.max_rect().bottom(),
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
            }

            if is_dragging || (ui.input().pointer.primary_down() && is_pointer_in_time_line_rect) {
                if let Some(time) = self.time_ranges_ui.time_from_x(pointer_pos.x) {
                    time_control.set_time(time);
                    ui.memory().set_dragged_id(time_drag_id);
                }
            }
        }

        parent_scroll_delta
    }

    fn paint_time_ranges_and_ticks(
        &mut self,
        ui: &mut egui::Ui,
        time_area_painter: &egui::Painter,
        y_range: RangeInclusive<f32>,
    ) {
        for (x_range, range) in &self.time_ranges_ui.ranges {
            let rect = Rect::from_x_y_ranges(x_range.clone(), y_range.clone());
            paint_time_range(
                ui,
                time_area_painter,
                &rect,
                range,
                self.time_ranges_ui.gap_width,
            );
        }

        if false {
            // visually separate the different ranges:
            use itertools::Itertools as _;
            for (a, b) in self.time_ranges_ui.ranges.iter().tuple_windows() {
                let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
                let x = lerp(*a.0.end()..=*b.0.start(), 0.5);
                let y_top = *y_range.start();
                let y_bottom = *y_range.end();
                time_area_painter.vline(x, y_top..=y_bottom, stroke);
            }
        }
    }

    fn tree_ui(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        time_area_painter: &egui::Painter,
        ui: &mut egui::Ui,
    ) {
        let mut path = vec![];
        self.show_tree(
            log_db,
            context,
            time_area_painter,
            &mut path,
            &log_db.object_tree,
            ui,
        );
    }

    fn show_tree(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        time_area_painter: &egui::Painter,
        path: &mut Vec<ObjectPathComponent>,
        tree: &ObjectTree,
        ui: &mut egui::Ui,
    ) {
        use egui::*;

        // TODO: ignore rows that have no data for the current time source?

        if tree.children.is_empty() && tree.times.is_empty() {
            return;
        }

        // how to show the time component.
        let text = if let Some(last) = path.last() {
            if tree.children.is_empty() {
                last.to_string()
            } else {
                format!("{}/", last)
            }
        } else {
            "/".to_string()
        };

        let indent = ui.spacing().indent;

        let mut also_show_child_points = true;

        let response = if tree.children.is_empty() {
            // leaf
            ui.horizontal(|ui| {
                // Add some spacing to match CollapsingHeader:
                ui.spacing_mut().item_spacing.x = 0.0;
                let response = ui.allocate_response(egui::vec2(indent, 0.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(response.rect.center(), 2.0, ui.visuals().text_color());
                ui.label(text);
            })
            .response
        } else {
            // node with more children
            let collapsing_response = egui::CollapsingHeader::new(text)
                .id_source(&path)
                .default_open(path.is_empty())
                .show(ui, |ui| {
                    self.show_children(log_db, context, time_area_painter, path, tree, ui);
                });

            let is_closed = collapsing_response.body_returned.is_none();
            also_show_child_points = is_closed; // if we are open, children show themselves
            collapsing_response.header_response
        };

        {
            // paint hline guide:
            let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            stroke.color = stroke.color.linear_multiply(0.5);
            let y = response.rect.bottom() + ui.spacing().item_spacing.y * 0.5;
            ui.painter()
                .hline(response.rect.left()..=ui.max_rect().right(), y, stroke);
        }

        self.next_col_right = self.next_col_right.max(response.rect.right());
        let (top_y, bottom_y) = (response.rect.top(), response.rect.bottom());

        // let center_y = response.rect.center().y;

        if true {
            response.on_hover_ui(|ui| {
                ui.label(ObjectPath(path.clone()).to_string());
                let summary = tree.data.summary();
                if !summary.is_empty() {
                    ui.label(summary);
                }
            });
        } else {
            response.on_hover_ui(|ui| {
                summary_of_tree(ui, path, tree);
            });
        }

        // show the data in the time area:
        {
            crate::profile_scope!("balls");
            let pointer_pos = ui.input().pointer.hover_pos();

            let source = if also_show_child_points {
                &tree.prefix_times
            } else {
                &tree.times
            };

            let mut hovered_messages = vec![];

            let mut scatter = BallScatterer::default();

            let hovered_color = ui.visuals().widgets.hovered.text_color();
            let inactive_color = ui
                .visuals()
                .widgets
                .inactive
                .text_color()
                .linear_multiply(0.75);

            for (time, log_id) in source {
                if let Some(time) = time.0.get(context.time_control.source()).copied() {
                    if let Some(x) = self.time_ranges_ui.x_from_time(time) {
                        let r = 2.0;
                        let pos = scatter.add(x, r, (top_y, bottom_y));

                        let is_hovered = pointer_pos
                            .map_or(false, |pointer_pos| pos.distance(pointer_pos) < 1.5 * r);

                        let mut color = if is_hovered {
                            hovered_color
                        } else {
                            inactive_color
                        };
                        if ui.visuals().dark_mode {
                            color = color.additive();
                        }

                        time_area_painter.circle_filled(pos, 2.0, color);

                        if is_hovered {
                            hovered_messages.push(*log_id);
                        }
                    }
                }
            }

            if !hovered_messages.is_empty() {
                egui::containers::popup::show_tooltip_at_pointer(
                    ui.ctx(),
                    Id::new("data_tooltip"),
                    |ui| {
                        // TODO: show as a table
                        for log_id in &hovered_messages {
                            if let Some(msg) = log_db.get_msg(log_id) {
                                ui.push_id(log_id, |ui| {
                                    ui.group(|ui| {
                                        crate::space_view::show_log_msg(
                                            context,
                                            ui,
                                            msg,
                                            crate::Preview::Small,
                                        );
                                    });
                                });
                            }
                        }
                    },
                );
            }
        }
    }

    fn show_children(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        time_area_painter: &egui::Painter,
        path: &mut Vec<ObjectPathComponent>,
        tree: &ObjectTree,
        ui: &mut egui::Ui,
    ) {
        for (name, node) in &tree.children {
            path.push(ObjectPathComponent::String(name.clone()));
            self.show_tree(
                log_db,
                context,
                time_area_painter,
                path,
                &node.string_children,
                ui,
            );
            path.pop();

            for (id, tree) in &node.persist_id_children {
                path.push(ObjectPathComponent::PersistId(name.clone(), id.clone()));
                self.show_tree(log_db, context, time_area_painter, path, tree, ui);
                path.pop();
            }

            for (id, tree) in &node.temp_id_children {
                path.push(ObjectPathComponent::PersistId(name.clone(), id.clone()));
                self.show_tree(log_db, context, time_area_painter, path, tree, ui);
                path.pop();
            }
        }
    }
}

fn top_row_ui(log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let time_control = &mut context.time_control;
        time_control.play_pause(&log_db.time_points, ui);
        ui.with_layout(egui::Layout::right_to_left(), |ui| {
            ui.colored_label(ui.visuals().widgets.inactive.text_color(), "Help!")
                .on_hover_text(
                    "Drag main area to pan.\n\
            Zoom: Ctrl/cmd + scroll, or drag up/down with secondary mouse button.\n\
            Double-click to reset view.\n\
            Press spacebar to pause/resume.",
                );

            if let Some(time) = time_control.time() {
                ui.vertical_centered(|ui| {
                    ui.monospace(time.to_string());
                });
            }
        });
    });
}

// ----------------------------------------------------------------------------

/// How much space on side of the data in the defaut view.
const SIDE_MARGIN: f32 = 8.0;

/// Sze of the gap between time segments.
fn gap_width(x_range: &RangeInclusive<f32>, segments: &[TimeRange]) -> f32 {
    let max_gap = 16.0;
    let num_gaps = segments.len().saturating_sub(1);
    if num_gaps == 0 {
        // gap width doesn't matter
        max_gap
    } else {
        let width = *x_range.end() - *x_range.start();
        (width / (4.0 * num_gaps as f32)).at_most(max_gap)
    }
}

/// Find a nice view of everything.
fn view_everything(x_range: &RangeInclusive<f32>, time_source_axis: &TimeSourceAxis) -> TimeView {
    let gap_width = gap_width(x_range, &time_source_axis.ranges);
    let num_gaps = time_source_axis.ranges.len().saturating_sub(1);
    let width = *x_range.end() - *x_range.start();
    let width_sans_gaps = width - num_gaps as f32 * gap_width;

    let factor = if width_sans_gaps > 0.0 {
        width / width_sans_gaps
    } else {
        1.0 // too narrow to fit everything anyways
    };

    let min = time_source_axis.min();
    let time_spanned = time_source_axis.sum_time_span() * factor as f64;

    // Leave some room on the margins:
    let time_margin = time_spanned * (SIDE_MARGIN / width.at_least(64.0)) as f64;
    let min = min.add_offset_f64(-time_margin);
    let time_spanned = time_spanned + 2.0 * time_margin;

    TimeView { min, time_spanned }
}

/// Recreated each frame.
struct TimeRangesUi {
    /// The total x-range we are viewing
    x_range: RangeInclusive<f32>,

    time_view: TimeView,
    /// x ranges matched to time ranges
    ranges: Vec<(RangeInclusive<f32>, TimeRange)>,

    /// x distance per time unit
    points_per_time: f32,

    gap_width: f32,
}

impl Default for TimeRangesUi {
    /// Safe, meaningless default
    fn default() -> Self {
        Self {
            x_range: 0.0..=1.0,
            time_view: TimeView {
                min: TimeValue::Sequence(0),
                time_spanned: 1.0,
            },
            ranges: vec![],
            points_per_time: 1.0,
            gap_width: 1.0,
        }
    }
}

impl TimeRangesUi {
    fn new(x_range: RangeInclusive<f32>, time_view: TimeView, segments: &[TimeRange]) -> Self {
        fn span(time_range: &TimeRange) -> f64 {
            time_range.span().unwrap_or_default()
        }

        //        <------- time_view ------>
        //        <-------- x_range ------->
        //        |                        |
        //    [segment] [long segment]
        //             ^ gap

        let gap_width = gap_width(&x_range, segments);
        let width = *x_range.end() - *x_range.start();
        let points_per_time = width / time_view.time_spanned as f32;
        let points_per_time = if points_per_time > 0.0 && points_per_time.is_finite() {
            points_per_time
        } else {
            1.0
        };

        let mut left = 0.0; // we will translate things left/right later
        let ranges = segments
            .iter()
            .map(|range| {
                let range_width = span(range) as f32 * points_per_time;
                let right = left + range_width;
                let x_range = left..=right;
                left = right + gap_width;
                (x_range, *range)
            })
            .collect();

        let mut slf = Self {
            x_range: x_range.clone(),
            time_view,
            ranges,
            points_per_time,
            gap_width,
        };

        if let Some(time_start_x) = slf.x_from_time(time_view.min) {
            // Now move things left/right to align `x_range` and `view_range`:
            let x_translate = *x_range.start() - time_start_x;
            for (range, _) in &mut slf.ranges {
                *range = (*range.start() + x_translate)..=(*range.end() + x_translate);
            }
        }

        slf
    }

    /// Make sure the time is not between ranges.
    fn snap_time(&self, value: TimeValue) -> TimeValue {
        for (_, range) in &self.ranges {
            if value < range.min {
                return range.min;
            } else if value <= range.max {
                return value;
            }
        }
        value
    }

    fn x_from_time(&self, needle_time: TimeValue) -> Option<f32> {
        let (first_x_range, first_time_range) = self.ranges.first()?;
        let mut last_x = *first_x_range.start();
        let mut last_time = first_time_range.min;

        if needle_time <= last_time {
            // extrapolate:
            return Some(
                last_x
                    - self.points_per_time * TimeRange::new(needle_time, last_time).span()? as f32,
            );
        }

        for (x_range, range) in &self.ranges {
            if needle_time < range.min {
                let t = TimeRange::new(last_time, range.min).lerp_t(needle_time)?;
                return Some(lerp(last_x..=*x_range.start(), t));
            } else if needle_time <= range.max {
                let t = range.lerp_t(needle_time)?;
                return Some(lerp(x_range.clone(), t));
            } else {
                last_x = *x_range.end();
                last_time = range.max;
            }
        }

        // extrapolate:
        Some(last_x + self.points_per_time * TimeRange::new(last_time, needle_time).span()? as f32)
    }

    fn time_from_x(&self, needle_x: f32) -> Option<TimeValue> {
        let (first_x_range, first_time_range) = self.ranges.first()?;
        let mut last_x = *first_x_range.start();
        let mut last_time = first_time_range.min;

        if needle_x <= last_x {
            // extrapolate:
            return Some(last_time.add_offset_f32((needle_x - last_x) / self.points_per_time));
        }

        for (x_range, range) in &self.ranges {
            if needle_x < *x_range.start() {
                let t = remap(needle_x, last_x..=*x_range.start(), 0.0..=1.0);
                return TimeRange::new(last_time, range.min).lerp(t);
            } else if needle_x <= *x_range.end() {
                let t = remap(needle_x, x_range.clone(), 0.0..=1.0);
                return range.lerp(t);
            } else {
                last_x = *x_range.end();
                last_time = range.max;
            }
        }

        // extrapolate:
        Some(last_time.add_offset_f32((needle_x - last_x) / self.points_per_time))
    }

    /// Pan the view, returning the new view.
    fn pan(&self, delta_x: f32) -> Option<TimeView> {
        Some(TimeView {
            min: self.time_from_x(*self.x_range.start() + delta_x)?,
            time_spanned: self.time_view.time_spanned,
        })
    }

    /// Zoom the view around the given x, returning the new view.
    fn zoom_at(&self, x: f32, zoom_factor: f32) -> Option<TimeView> {
        let mut min_x = *self.x_range.start();
        let max_x = *self.x_range.end();
        let t = remap(x, min_x..=max_x, 0.0..=1.0);

        let width = max_x - min_x;

        let new_width = width / zoom_factor;
        let width_delta = new_width - width;

        min_x -= t * width_delta;

        Some(TimeView {
            min: self.time_from_x(min_x)?,
            time_spanned: self.time_view.time_spanned / zoom_factor as f64,
        })
    }
}

fn paint_time_range(
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    rect: &Rect,
    range: &TimeRange,
    gap_width: f32,
) {
    let bg_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    let fg_stroke = ui.visuals().widgets.noninteractive.fg_stroke;

    time_area_painter.rect_filled(
        rect.expand2(vec2(gap_width / 4.0, 0.0)), // give zero-width time segments some width
        3.0,
        bg_stroke.color.linear_multiply(0.5),
    );

    let (min, max) = (range.min, range.max);
    if let (TimeValue::Time(min), TimeValue::Time(max)) = (min, max) {
        if min != max {
            // TODO: handle different time spans better
            if max - min < log_types::Duration::from_secs(20.0) {
                let mut ns = min.nanos_since_epoch();
                let small_step_size_ns = 100_000_000;
                ns = ((ns - 1) / small_step_size_ns + 1) * small_step_size_ns;
                while ns <= max.nanos_since_epoch() {
                    let x = lerp(
                        rect.x_range(),
                        range.lerp_t(Time::from_ns_since_epoch(ns).into()).unwrap(),
                    );

                    let bottom = if ns % (10 * small_step_size_ns) == 0 {
                        // full second
                        rect.bottom()
                    } else {
                        // tenth
                        lerp(rect.y_range(), 0.25)
                    };

                    time_area_painter.vline(x, rect.top()..=bottom, fg_stroke);

                    ns += small_step_size_ns;
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------

fn summary_of_tree(ui: &mut egui::Ui, path: &mut Vec<ObjectPathComponent>, tree: &ObjectTree) {
    egui::Grid::new("summary_of_children")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            summary_of_children(ui, path, tree);
        });
}

fn summary_of_children(ui: &mut egui::Ui, path: &mut Vec<ObjectPathComponent>, tree: &ObjectTree) {
    ui.label(ObjectPath(path.clone()).to_string());
    ui.label(tree.data.summary());
    ui.end_row();

    for (name, node) in &tree.children {
        path.push(ObjectPathComponent::String(name.clone()));
        summary_of_children(ui, path, &node.string_children);
        path.pop();

        for (id, tree) in &node.persist_id_children {
            path.push(ObjectPathComponent::PersistId(name.clone(), id.clone()));
            summary_of_children(ui, path, tree);
            path.pop();
        }

        for (id, tree) in &node.temp_id_children {
            path.push(ObjectPathComponent::PersistId(name.clone(), id.clone()));
            summary_of_children(ui, path, tree);
            path.pop();
        }
    }
}

// ----------------------------------------------------------------------------

/// Positions circles on a horizontal line with some vertical scattering to avoid overlap.
struct BallScatterer {
    recent: [Pos2; Self::MEMORY_SIZE],
    cursor: usize,
}

impl Default for BallScatterer {
    fn default() -> Self {
        Self {
            recent: [Pos2::new(f32::INFINITY, f32::INFINITY); Self::MEMORY_SIZE],
            cursor: 0,
        }
    }
}

impl BallScatterer {
    const MEMORY_SIZE: usize = 8;

    pub fn add(&mut self, x: f32, r: f32, (min_y, max_y): (f32, f32)) -> Pos2 {
        let min_y = min_y + r; // some padding
        let max_y = max_y - r; // some padding

        let r2 = r * r * 3.0; // allow some overlap

        let center_y = 0.5 * (min_y + max_y);
        let mut best_free_y = f32::INFINITY;
        let mut best_colliding_y = center_y;
        let mut best_colliding_d2 = 0.0;

        for y_offset in 0..=(max_y - min_y).round() as i32 {
            let y = min_y + y_offset as f32;
            let d2 = self.closest_dist_sq(&pos2(x, y));
            let intersects = d2 < r2;
            if intersects {
                // pick least colliding
                if d2 > best_colliding_d2 {
                    best_colliding_y = y;
                    best_colliding_d2 = d2;
                }
            } else {
                // pick closest to center
                if (y - center_y).abs() < (best_free_y - center_y).abs() {
                    best_free_y = y;
                }
            }
        }

        let y = if best_free_y.is_finite() {
            best_free_y
        } else {
            best_colliding_y
        };

        let pos = pos2(x, y);
        self.recent[self.cursor] = pos;
        self.cursor = (self.cursor + 1) % Self::MEMORY_SIZE;
        pos
    }

    fn closest_dist_sq(&self, pos: &Pos2) -> f32 {
        let mut d2 = f32::INFINITY;
        for recent in &self.recent {
            d2 = d2.min(recent.distance_sq(*pos));
        }
        d2
    }
}

// ----------------------------------------------------------------------------

/// fade left/right sides of time-area, because it looks nice:
fn fade_sides(ui: &mut egui::Ui, time_area: Rect) {
    let fade_width = SIDE_MARGIN - 1.0;

    let base_rect = time_area.expand(0.5); // expand slightly to cover feathering.

    let window_fill = ui.visuals().window_fill();
    let mut left_rect = base_rect;

    left_rect.set_right(left_rect.left() + fade_width);
    ui.painter()
        .add(fade_mesh(left_rect, [window_fill, Color32::TRANSPARENT]));

    let mut right_rect = base_rect;
    right_rect.set_left(right_rect.right() - fade_width);
    ui.painter()
        .add(fade_mesh(right_rect, [Color32::TRANSPARENT, window_fill]));
}

fn fade_mesh(rect: Rect, [left_color, right_color]: [Color32; 2]) -> egui::Mesh {
    use egui::epaint::Vertex;
    let mut mesh = egui::Mesh::default();

    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(2, 1, 3);

    mesh.vertices.push(Vertex {
        pos: rect.left_top(),
        uv: egui::epaint::WHITE_UV,
        color: left_color,
    });
    mesh.vertices.push(Vertex {
        pos: rect.right_top(),
        uv: egui::epaint::WHITE_UV,
        color: right_color,
    });
    mesh.vertices.push(Vertex {
        pos: rect.left_bottom(),
        uv: egui::epaint::WHITE_UV,
        color: left_color,
    });
    mesh.vertices.push(Vertex {
        pos: rect.right_bottom(),
        uv: egui::epaint::WHITE_UV,
        color: right_color,
    });

    mesh
}
