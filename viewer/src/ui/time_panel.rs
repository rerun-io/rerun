use std::ops::RangeInclusive;

use crate::time_axis::TimeSourceAxes;
use crate::TimeControl;
use crate::ViewerContext;
use crate::{log_db::ObjectTree, time_axis::TimeSegment, LogDb};
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
    time_segments_ui: TimeSegmentsUi,
}

impl Default for TimePanel {
    fn default() -> Self {
        Self {
            prev_col_width: 400.0,
            next_col_right: 0.0,
            time_segments_ui: Default::default(),
        }
    }
}

impl TimePanel {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        crate::profile_function!();

        self.next_col_right = ui.min_rect().left(); // this will expand during the call

        // Where the time will be shown.
        let time_x_range = {
            let left = ui.min_rect().left() + self.prev_col_width;
            let right = ui.max_rect().right() - ui.spacing().scroll_bar_width - 8.0;
            left..=right
        };

        // play control and current time
        ui.horizontal(|ui| {
            let time_control = &mut context.time_control;
            time_control.play_pause(&log_db.time_points, ui);
            if let Some(time) = time_control.time() {
                ui.vertical_centered(|ui| {
                    ui.monospace(time.to_string());
                });
            }
        });

        let time_area = Rect::from_x_y_ranges(
            time_x_range.clone(),
            ui.min_rect().bottom()..=ui.max_rect().bottom(),
        );

        ui.horizontal(|ui| {
            self.time_row_ui(log_db, context, ui, time_x_range.clone());
        });

        ui.separator();

        // all the object rows:
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                crate::profile_scope!("tree_ui");
                self.tree_ui(log_db, context, ui);
            });

        let time_control = &mut context.time_control;

        self.click_to_select_time(time_control, ui, &time_area);

        if let Some(time) = time_control.time() {
            // so time doesn't get stuck between non-continuos regions
            let time = self.time_segments_ui.snap_time(time);
            time_control.set_time(time);
        }

        // remember where to show the time for next frame:
        let margin = 16.0;
        self.prev_col_width = self.next_col_right - ui.min_rect().left() + margin;
    }

    fn click_to_select_time(
        &mut self,
        time_control: &mut TimeControl,
        ui: &mut egui::Ui,
        time_area: &Rect,
    ) {
        let pointer = ui.input().pointer.hover_pos();

        let time_drag_id = ui.id().with("time_drag_id");

        let mut is_hovering = false;
        let mut is_dragging = ui.memory().is_being_dragged(time_drag_id);

        // show current time as a line:
        if let Some(time) = time_control.time() {
            if let Some(x) = self.time_segments_ui.x_from_time(time) {
                if let Some(pointer) = pointer {
                    let line_rect = Rect::from_x_y_ranges(x..=x, time_area.y_range());

                    is_hovering = line_rect.distance_to_pos(pointer)
                        <= ui.style().interaction.resize_grab_radius_side;

                    if ui.input().pointer.any_pressed()
                        && ui.input().pointer.any_down()
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
                    pos2(x - 0.5 * w, time_area.top()), // left top
                    pos2(x + 0.5 * w, time_area.top()), // right top
                    pos2(x, time_area.top() + w),       // bottom
                ];
                ui.painter().add(egui::Shape::convex_polygon(
                    triangle,
                    stroke.color,
                    egui::Stroke::none(),
                ));
                ui.painter()
                    .vline(x, (time_area.top() + w)..=time_area.bottom(), stroke);
            }
        }

        // Show preview: "click here to view time here"
        let pointer = ui.input().pointer.hover_pos();
        if let Some(pointer) = pointer {
            if !is_hovering && !is_dragging && time_area.contains(pointer) {
                ui.painter().vline(
                    pointer.x,
                    time_area.top()..=ui.max_rect().bottom(),
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
            }

            if is_dragging || (ui.input().pointer.any_down() && time_area.contains(pointer)) {
                if let Some(time) = self.time_segments_ui.time_from_x(pointer.x) {
                    time_control.set_time(time);
                    ui.memory().set_dragged_id(time_drag_id);
                }
            }
        }
    }

    fn time_row_ui(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        ui: &mut egui::Ui,
        time_x_range: RangeInclusive<f32>,
    ) {
        crate::profile_function!();
        let time_source_axes = TimeSourceAxes::new(&log_db.time_points);
        if let Some(segments) = time_source_axes.sources.get(context.time_control.source()) {
            self.time_segments_ui = TimeSegmentsUi::new(time_x_range, &segments.segments);
        } else {
            self.time_segments_ui = Default::default();
        }

        let y_range = self.time_source_ui(log_db, context, ui).rect.y_range();
        self.time_axis_ui(ui, y_range);
    }

    fn time_source_ui(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        let response = context
            .time_control
            .time_source_selector(&log_db.time_points, ui);
        self.next_col_right = self.next_col_right.max(response.rect.right());
        response
    }

    fn time_axis_ui(&mut self, ui: &mut egui::Ui, y_range: RangeInclusive<f32>) {
        for (x_range, segment) in &self.time_segments_ui.ranges {
            let rect = Rect::from_x_y_ranges(x_range.clone(), y_range.clone());
            paint_time_segment(ui, &rect, segment);
        }

        if false {
            // visually separate the different segments:
            use itertools::Itertools as _;
            for (a, b) in self.time_segments_ui.ranges.iter().tuple_windows() {
                let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
                let x = lerp(*a.0.end()..=*b.0.start(), 0.5);
                let y_top = *y_range.start();
                let y_bottom = *y_range.end();
                ui.painter().vline(x, y_top..=y_bottom, stroke);
            }
        }
    }

    fn tree_ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        let mut path = vec![];
        self.show_tree(log_db, context, &mut path, &log_db.object_tree, ui);
    }

    fn show_tree(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
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
                    self.show_children(log_db, context, path, tree, ui);
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
                    if let Some(x) = self.time_segments_ui.x_from_time(time) {
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

                        ui.painter().circle_filled(pos, 2.0, color);

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
        path: &mut Vec<ObjectPathComponent>,
        tree: &ObjectTree,
        ui: &mut egui::Ui,
    ) {
        for (name, node) in &tree.children {
            path.push(ObjectPathComponent::String(name.clone()));
            self.show_tree(log_db, context, path, &node.string_children, ui);
            path.pop();

            for (id, tree) in &node.persist_id_children {
                path.push(ObjectPathComponent::PersistId(name.clone(), id.clone()));
                self.show_tree(log_db, context, path, tree, ui);
                path.pop();
            }

            for (id, tree) in &node.temp_id_children {
                path.push(ObjectPathComponent::PersistId(name.clone(), id.clone()));
                self.show_tree(log_db, context, path, tree, ui);
                path.pop();
            }
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Default)]
struct TimeSegmentsUi {
    /// x ranges matched to time segments
    ranges: Vec<(RangeInclusive<f32>, TimeSegment)>,
}

impl TimeSegmentsUi {
    fn new(x_range: RangeInclusive<f32>, time_segments: &[TimeSegment]) -> Self {
        if time_segments.is_empty() {
            Self { ranges: vec![] }
        } else if time_segments.len() == 1 {
            Self {
                ranges: vec![(x_range, time_segments[0].clone())],
            }
        } else {
            fn span(time_segment: &TimeSegment) -> f64 {
                time_segment.span().unwrap_or_default()
            }

            // figure out how much space to allocate to each time segment.
            // this approach does not support zooming.
            // it is also quite ad-hoc and can be improved.
            let width = *x_range.end() - *x_range.start();
            let min_segment_length = (width / (time_segments.len() * 2) as f32).at_most(16.0);
            let remaining = width - min_segment_length * time_segments.len() as f32;
            let margin = (remaining / (time_segments.len() - 1) as f32).at_most(8.0);
            let remaining = remaining - margin * (time_segments.len() - 1) as f32;

            let span_sum: f64 = time_segments.iter().map(span).sum();
            let points_per_span = remaining / span_sum as f32;

            let mut left = *x_range.start();
            let mut ranges = vec![];

            for segment in time_segments {
                let segment_width = min_segment_length + span(segment) as f32 * points_per_span;
                let right = left + segment_width;
                ranges.push(((left..=right), segment.clone()));
                left = right + margin;
            }
            Self { ranges }
        }
    }

    /// Make sure the time is not between segments.
    fn snap_time(&self, value: TimeValue) -> TimeValue {
        for (_, segment) in &self.ranges {
            if value < segment.min {
                return segment.min;
            } else if value <= segment.max {
                return value;
            }
        }
        value
    }

    fn x_from_time(&self, value: TimeValue) -> Option<f32> {
        let (first_x_range, first_segment) = self.ranges.first()?;
        let mut last_x = *first_x_range.start();
        let mut last_time = first_segment.min;

        for (x_range, segment) in &self.ranges {
            if value < segment.min {
                let t = value.lerp_t(last_time..=segment.min)?;
                return Some(lerp(x_range.clone(), t));
            } else if value <= segment.max {
                let t = segment.lerp_t(value)?;
                return Some(lerp(x_range.clone(), t));
            } else {
                last_x = *x_range.end();
                last_time = segment.max;
            }
        }

        Some(last_x)
    }

    fn time_from_x(&self, x: f32) -> Option<TimeValue> {
        let (first_x_range, first_segment) = self.ranges.first()?;
        let mut last_x = *first_x_range.start();
        let mut last_time = first_segment.min;

        for (x_range, segment) in &self.ranges {
            if x < *x_range.start() {
                let t = remap(x, last_x..=*x_range.start(), 0.0..=1.0);
                return TimeValue::lerp(last_time..=segment.min, t);
            } else if x <= *x_range.end() {
                let t = remap(x, x_range.clone(), 0.0..=1.0);
                return TimeValue::lerp(segment.min..=segment.max, t);
            } else {
                last_x = *x_range.end();
                last_time = segment.max;
            }
        }

        Some(last_time)
    }
}

fn paint_time_segment(ui: &mut egui::Ui, rect: &Rect, segment: &TimeSegment) {
    let bg_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    let fg_stroke = ui.visuals().widgets.noninteractive.fg_stroke;
    ui.painter()
        .rect_filled(*rect, 3.0, bg_stroke.color.linear_multiply(0.5));

    let (min, max) = (segment.min, segment.max);
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
                        segment
                            .lerp_t(Time::from_ns_since_epoch(ns).into())
                            .unwrap(),
                    );

                    let bottom = if ns % (10 * small_step_size_ns) == 0 {
                        // full second
                        rect.bottom()
                    } else {
                        // tenth
                        lerp(rect.y_range(), 0.25)
                    };

                    ui.painter().vline(x, rect.top()..=bottom, fg_stroke);

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
