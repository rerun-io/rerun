use std::{
    collections::{BTreeMap, BTreeSet},
    ops::RangeInclusive,
};

use crate::{
    log_db::ObjectTree, misc::time_axis::TimeSourceAxis, misc::time_control::TimeSelectionType,
    LogDb, TimeControl, TimeRange, TimeView, ViewerContext,
};

use egui::*;
use re_log_types::*;

/// A column where we should button to hide/show a propery.
const PROPERY_COLUMN_WIDTH: f32 = 14.0;

/// A panel that shows objects to the left, time on the top.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TimePanel {
    /// Width of the object name columns previous frame.
    prev_col_width: f32,

    /// Set at the start of the frame
    #[serde(skip)]
    propery_column_x_range: RangeInclusive<f32>,

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
            propery_column_x_range: 0.0..=0.0,
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

        let prop_column_left =
            ui.min_rect().left() + self.prev_col_width + ui.spacing().item_spacing.x;
        let prop_column_right = prop_column_left + PROPERY_COLUMN_WIDTH;
        self.propery_column_x_range = prop_column_left..=prop_column_right;
        let time_x_left = prop_column_right + ui.spacing().item_spacing.x;

        // Where the time will be shown.
        let time_x_range = {
            let right =
                ui.max_rect().right() - ui.spacing().scroll_bar_width - ui.spacing().item_spacing.x;
            time_x_left..=right
        };

        self.time_ranges_ui = initialize_time_ranges_ui(log_db, context, time_x_range.clone());

        // includes the time selection and time ticks rows.
        let time_area = Rect::from_x_y_ranges(
            time_x_range.clone(),
            ui.min_rect().bottom()..=ui.max_rect().bottom(),
        );

        let time_selection_rect = {
            let response = ui
                .horizontal(|ui| {
                    context.time_control.selection_ui(ui);
                })
                .response;
            self.next_col_right = self.next_col_right.max(response.rect.right());
            let y_range = response.rect.y_range();
            Rect::from_x_y_ranges(time_x_range.clone(), y_range)
        };

        let time_line_rect = {
            let response = ui
                .horizontal(|ui| {
                    context.time_control.play_pause_ui(&log_db.time_points, ui);
                })
                .response;

            self.next_col_right = self.next_col_right.max(response.rect.right());
            let y_range = response.rect.y_range();
            Rect::from_x_y_ranges(time_x_range.clone(), y_range)
        };

        let time_area_painter = ui.painter().with_clip_rect(time_area);

        ui.painter()
            .rect_filled(time_area, 1.0, ui.visuals().extreme_bg_color);

        ui.separator();

        paint_time_ranges_and_ticks(
            &self.time_ranges_ui,
            ui,
            &time_area_painter,
            time_selection_rect.top()..=time_line_rect.bottom(),
            // time_line_rect.y_range(),
            time_line_rect.top()..=time_area.bottom(),
            context.time_control.time_type(),
        );
        time_selection_ui(
            &self.time_ranges_ui,
            &mut context.time_control,
            ui,
            &time_area_painter,
            &time_selection_rect,
        );
        time_marker_ui(
            &self.time_ranges_ui,
            &mut context.time_control,
            ui,
            &time_area_painter,
            &time_line_rect,
            time_area.bottom(),
        );
        let scroll_delta = interact_with_time_area(
            &self.time_ranges_ui,
            &mut context.time_control,
            ui,
            &time_area,
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

        // TODO(emilk): fix problem of the fade covering the hlines. Need Shape Z values! https://github.com/emilk/egui/issues/1516
        if true {
            fade_sides(ui, time_area);
        }

        self.time_ranges_ui.snap_time_control(context);

        // remember where to show the time for next frame:
        self.prev_col_width = self.next_col_right - ui.min_rect().left();
    }

    fn tree_ui(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        time_area_painter: &egui::Painter,
        ui: &mut egui::Ui,
    ) {
        let mut path = vec![];
        self.show_children(
            log_db,
            context,
            time_area_painter,
            &mut path,
            &log_db.data_tree,
            ui,
        );
    }

    fn show_tree(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        time_area_painter: &egui::Painter,
        path: &mut Vec<ObjPathComp>,
        tree: &ObjectTree,
        ui: &mut egui::Ui,
    ) {
        use egui::*;

        // TODO(emilk): ignore rows that have no data for the current time source?

        let obj_path = ObjPath::from(&ObjPathBuilder::new(path.clone()));

        // The last part of the the path component
        let text = if let Some(last) = path.last() {
            if tree.is_leaf() {
                last.to_string()
            } else {
                format!("{}/", last) // show we have children with a /
            }
        } else {
            "/".to_owned()
        };

        let collapsing_response = egui::CollapsingHeader::new(text)
            .id_source(&path)
            .default_open(path.is_empty()) //  || (path.len() == 1 && tree.children.len() < 3)) TODO(emilk) when data path has been simplified
            .show(ui, |ui| {
                self.show_children(log_db, context, time_area_painter, path, tree, ui);
            });

        let is_closed = collapsing_response.body_returned.is_none();
        let response = collapsing_response.header_response;

        {
            // paint hline guide:
            let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            stroke.color = stroke.color.linear_multiply(0.5);
            let left = response.rect.left() + ui.spacing().indent;
            let y = response.rect.bottom() + ui.spacing().item_spacing.y * 0.5;
            ui.painter().hline(left..=ui.max_rect().right(), y, stroke);
        }

        self.next_col_right = self.next_col_right.max(response.rect.right());

        let response = response.on_hover_ui(|ui| {
            ui.label(obj_path.to_string());
        });

        // ----------------------------------------------
        // Property column:

        {
            let are_all_ancestors_visible = obj_path.is_root()
                || context
                    .projected_object_properties
                    .get(&obj_path.parent())
                    .visible;

            let mut props = context.individual_object_properties.get(&obj_path);
            let property_rect =
                Rect::from_x_y_ranges(self.propery_column_x_range.clone(), response.rect.y_range());
            let mut ui = ui.child_ui(
                property_rect,
                egui::Layout::left_to_right(egui::Align::Center),
            );
            ui.set_enabled(are_all_ancestors_visible);
            ui.toggle_value(&mut props.visible, "👁")
                .on_hover_text("Toggle visibility");
            context.individual_object_properties.set(obj_path, props);
        }

        // ----------------------------------------------

        // show the data in the time area:

        let full_width_rect = Rect::from_x_y_ranges(
            response.rect.left()..=ui.max_rect().right(),
            response.rect.y_range(),
        );

        if ui.is_rect_visible(full_width_rect) && is_closed {
            show_data_over_time(
                log_db,
                context,
                time_area_painter,
                ui,
                &tree.prefix_times,
                full_width_rect,
                &self.time_ranges_ui,
            );
        }
    }

    fn show_children(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        time_area_painter: &egui::Painter,
        path: &mut Vec<ObjPathComp>,
        tree: &ObjectTree,
        ui: &mut egui::Ui,
    ) {
        for (name, child) in &tree.string_children {
            path.push(ObjPathComp::String(*name));
            self.show_tree(log_db, context, time_area_painter, path, child, ui);
            path.pop();
        }
        for (index, child) in &tree.index_children {
            path.push(ObjPathComp::Index(index.clone()));
            self.show_tree(log_db, context, time_area_painter, path, child, ui);
            path.pop();
        }

        // If this is an object:
        if !tree.fields.is_empty() {
            let indent = ui.spacing().indent;

            let obj_path = ObjPath::from(ObjPathBuilder::new(path.clone()));
            for (field_name, data) in &tree.fields {
                let data_path = DataPath::new(obj_path.clone(), *field_name);

                let response = ui
                    .horizontal(|ui| {
                        // Add some spacing to match CollapsingHeader:
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let response =
                            ui.allocate_response(egui::vec2(indent, 0.0), egui::Sense::hover());
                        ui.painter().circle_filled(
                            response.rect.center(),
                            2.0,
                            ui.visuals().text_color(),
                        );
                        context.data_path_button_to(ui, field_name.as_str(), &data_path);
                    })
                    .response;

                {
                    // paint hline guide:
                    let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
                    stroke.color = stroke.color.linear_multiply(0.5);
                    let left = response.rect.left() + ui.spacing().indent;
                    let y = response.rect.bottom() + ui.spacing().item_spacing.y * 0.5;
                    ui.painter().hline(left..=ui.max_rect().right(), y, stroke);
                }

                let response = response.on_hover_ui(|ui| {
                    ui.label(data_path.to_string());
                    let summary = data.summary();
                    if !summary.is_empty() {
                        ui.label(summary);
                    }
                });

                // show the data in the time area:

                let full_width_rect = Rect::from_x_y_ranges(
                    response.rect.left()..=ui.max_rect().right(),
                    response.rect.y_range(),
                );

                if ui.is_rect_visible(full_width_rect) {
                    show_data_over_time(
                        log_db,
                        context,
                        time_area_painter,
                        ui,
                        &data.times,
                        full_width_rect,
                        &self.time_ranges_ui,
                    );
                }
            }
        }
    }
}

fn top_row_ui(log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        context
            .time_control
            .time_source_selector_ui(&log_db.time_points, ui);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            crate::misc::help_hover_button(ui).on_hover_text(
                "Drag main area to pan.\n\
            Zoom: Ctrl/cmd + scroll, or drag up/down with secondary mouse button.\n\
            Double-click to reset view.\n\
            Press spacebar to pause/resume.",
            );

            if let Some(range) = context.time_control.time_range() {
                let time_type = context.time_control.time_type();

                ui.vertical_centered(|ui| {
                    if range.min == range.max {
                        ui.monospace(TimeValue::new(time_type, range.min).to_string());
                    } else {
                        ui.monospace(format!(
                            "{} - {}",
                            TimeValue::new(time_type, range.min),
                            TimeValue::new(time_type, range.max)
                        ));
                    }
                });
            }
        });
    });
}

fn show_data_over_time(
    log_db: &LogDb,
    context: &mut ViewerContext,
    time_area_painter: &egui::Painter,
    ui: &mut egui::Ui,
    source: &BTreeMap<TimePoint, BTreeSet<LogId>>,
    full_width_rect: Rect,
    time_ranges_ui: &TimeRangesUi,
) {
    crate::profile_function!();

    // painting each data point as a separate circle is slow (too many circles!)
    // so we join time points that are close together.
    let points_per_time = time_ranges_ui.points_per_time().unwrap_or(f32::INFINITY);
    let max_stretch_length_in_time = 1.0 / points_per_time as f64; // TODO(emilk)

    let pointer_pos = ui.input().pointer.hover_pos();

    let hovered_color = ui.visuals().widgets.hovered.text_color();
    let inactive_color = ui
        .visuals()
        .widgets
        .inactive
        .text_color()
        .linear_multiply(0.75);

    let selected_time_range = if !context.time_control.selection_active {
        None
    } else {
        context.time_control.time_selection()
    };
    let time_source = *context.time_control.source();

    struct Stretch<'a> {
        start_x: f32,
        start_time: TimeInt,
        stop_time: TimeInt,
        selected: bool,
        log_ids: Vec<&'a BTreeSet<LogId>>,
    }

    let mut shapes = vec![];
    let mut scatter = BallScatterer::default();
    let mut hovered_messages = vec![];

    let mut paint_stretch = |stretch: &Stretch<'_>| {
        let stop_x = time_ranges_ui
            .x_from_time(stretch.stop_time)
            .unwrap_or(stretch.start_x);

        let num_messages: usize = stretch.log_ids.iter().map(|l| l.len()).sum();
        let radius = 2.5 * (1.0 + 0.5 * (num_messages as f32).log10());
        let radius = radius.at_most(full_width_rect.height() / 3.0);

        let x = (stretch.start_x + stop_x) * 0.5;
        let pos = scatter.add(x, radius, (full_width_rect.top(), full_width_rect.bottom()));

        let is_hovered = pointer_pos.map_or(false, |pointer_pos| {
            pos.distance(pointer_pos) < radius + 1.0
        });

        let mut color = if is_hovered {
            hovered_color
        } else {
            inactive_color
        };
        if ui.visuals().dark_mode {
            color = color.additive();
        }

        let radius = if is_hovered {
            1.75 * radius
        } else if stretch.selected {
            1.25 * radius
        } else {
            radius
        };

        shapes.push(Shape::circle_filled(pos, radius, color));

        if is_hovered && !ui.ctx().memory().is_anything_being_dragged() {
            hovered_messages.extend(stretch.log_ids.iter().copied().flatten().copied());
        }
    };

    let mut stretch: Option<Stretch<'_>> = None;

    for (time, log_ids) in source {
        // TODO(emilk): avoid this lookup by pre-partitioning on time source
        if let Some(time) = time.0.get(&time_source).copied() {
            let time = time.to_int();

            let selected = selected_time_range.map_or(true, |range| range.contains(time));

            if let Some(current_stretch) = &mut stretch {
                if current_stretch.selected == selected
                    && (time - current_stretch.start_time).to_f64() < max_stretch_length_in_time
                {
                    // extend:
                    current_stretch.stop_time = time;
                    current_stretch.log_ids.push(log_ids);
                } else {
                    // stop the previous…
                    paint_stretch(current_stretch);

                    stretch = None;
                }
            }

            if stretch.is_none() {
                if let Some(x) = time_ranges_ui.x_from_time(time) {
                    stretch = Some(Stretch {
                        start_x: x,
                        start_time: time,
                        stop_time: time,
                        selected,
                        log_ids: vec![log_ids],
                    });
                }
            }
        }
    }

    if let Some(stretch) = stretch {
        paint_stretch(&stretch);
    }

    time_area_painter.extend(shapes);

    if !hovered_messages.is_empty() {
        show_log_ids_tooltip(log_db, context, ui.ctx(), &hovered_messages);
    }
}

fn show_log_ids_tooltip(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ctx: &egui::Context,
    log_ids: &[LogId],
) {
    show_tooltip_at_pointer(ctx, Id::new("data_tooltip"), |ui| {
        // TODO(emilk): show as a table?
        if log_ids.len() == 1 {
            let log_id = log_ids[0];
            if let Some(msg) = log_db.get_data_msg(&log_id) {
                ui.push_id(log_id, |ui| {
                    ui.group(|ui| {
                        crate::space_view::show_log_msg(context, ui, msg, crate::Preview::Small);
                    });
                });
            }
        } else {
            ui.label(format!("{} messages", log_ids.len()));
        }
    });
}

// ----------------------------------------------------------------------------

fn initialize_time_ranges_ui(
    log_db: &LogDb,
    context: &mut ViewerContext,
    time_x_range: RangeInclusive<f32>,
) -> TimeRangesUi {
    crate::profile_function!();
    if let Some(time_points) = log_db.time_points.0.get(context.time_control.source()) {
        let time_source_axis = TimeSourceAxis::new(context.time_control.time_type(), time_points);
        let time_view = context.time_control.time_view();
        let time_view =
            time_view.unwrap_or_else(|| view_everything(&time_x_range, &time_source_axis));

        TimeRangesUi::new(time_x_range, time_view, &time_source_axis.ranges)
    } else {
        Default::default()
    }
}

fn paint_time_ranges_and_ticks(
    time_ranges_ui: &TimeRangesUi,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    line_y_range: RangeInclusive<f32>,
    segment_y_range: RangeInclusive<f32>,
    time_type: TimeType,
) {
    for segment in &time_ranges_ui.segments {
        let bg_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let rect = Rect::from_x_y_ranges(segment.x.clone(), segment_y_range.clone());
        time_area_painter.rect_filled(rect, 1.0, bg_stroke.color.linear_multiply(0.5));

        let rect = Rect::from_x_y_ranges(segment.x.clone(), line_y_range.clone());
        paint_time_range_ticks(ui, time_area_painter, &rect, time_type, &segment.time);
    }

    if false {
        // visually separate the different ranges:
        use itertools::Itertools as _;
        for (a, b) in time_ranges_ui.segments.iter().tuple_windows() {
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            let x = lerp(*a.x.end()..=*b.x.start(), 0.5);
            let y_top = *segment_y_range.start();
            let y_bottom = *segment_y_range.end();
            time_area_painter.vline(x, y_top..=y_bottom, stroke);
        }
    }
}

/// Returns a scroll delta
fn interact_with_time_area(
    time_ranges_ui: &TimeRangesUi,
    time_control: &mut TimeControl,
    ui: &mut egui::Ui,
    full_rect: &Rect,
) -> Vec2 {
    let pointer_pos = ui.input().pointer.hover_pos();

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
        if let Some(new_view_range) = time_ranges_ui.pan(-delta_x) {
            time_control.set_time_view(new_view_range);
        }
    }

    if zoom_factor != 1.0 {
        if let Some(pointer_pos) = pointer_pos {
            if let Some(new_view_range) = time_ranges_ui.zoom_at(pointer_pos.x, zoom_factor) {
                time_control.set_time_view(new_view_range);
            }
        }
    }

    if response.double_clicked() {
        time_control.reset_time_view();
    }

    parent_scroll_delta
}

fn initial_time_selection(time_ranges_ui: &TimeRangesUi, time_type: TimeType) -> Option<TimeRange> {
    let ranges = &time_ranges_ui.segments;

    // Try to find a long duration first, then fall back to shorter
    for min_duration in [2.0, 0.5, 0.0] {
        for segment in ranges {
            let range = &segment.tight_time;
            if range.min < range.max {
                match time_type {
                    TimeType::Time => {
                        let seconds = Duration::from(range.max - range.min).as_secs_f64();
                        if seconds > min_duration {
                            let one_sec = TimeInt::from(Duration::from_secs(1.0));
                            return Some(TimeRange::new(range.min, range.min + one_sec));
                        }
                    }
                    TimeType::Sequence => {
                        return Some(TimeRange::new(
                            range.min,
                            range.min + TimeInt::from((range.max - range.min).to_i64() / 2),
                        ));
                    }
                }
            }
        }
    }

    // all ranges have just a single data point in it. sight

    if ranges.len() < 2 {
        None // not enough to show anything meaningful
    } else {
        let end = (ranges.len() / 2).at_least(1);
        Some(TimeRange::new(
            ranges[0].tight_time.min,
            ranges[end].tight_time.max,
        ))
    }
}

fn time_selection_ui(
    time_ranges_ui: &TimeRangesUi,
    time_control: &mut TimeControl,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    rect: &Rect,
) {
    if time_control.time_selection().is_none() {
        // Helpfully select a time slice so that there always is a selection.
        // This helps new users ("what is that?").
        if let Some(selection) = initial_time_selection(time_ranges_ui, time_control.time_type()) {
            time_control.set_time_selection(selection);
        }
    }

    if time_control.time_selection().is_none() {
        time_control.selection_active = false;
    }

    // TODO(emilk): click to toggle on/off
    // when off, you cannot modify, just drag out a new one.

    let selection_color = time_control.selection_type.color(ui.visuals());

    let mut did_interact = false;

    let is_active = time_control.selection_active;

    let pointer_pos = ui.input().pointer.hover_pos();
    let is_pointer_in_rect = pointer_pos.map_or(false, |pointer_pos| rect.contains(pointer_pos));

    let left_edge_id = ui.id().with("selection_left_edge");
    let right_edge_id = ui.id().with("selection_right_edge");
    let move_id = ui.id().with("selection_move");

    let interact_radius = ui.style().interaction.resize_grab_radius_side;

    let mut is_hovering_existing = false;

    let transparent = if ui.visuals().dark_mode { 0.06 } else { 0.3 };

    // Paint existing selection and detect drag starting and hovering:
    if let Some(selected_range) = time_control.time_selection() {
        let min_x = time_ranges_ui.x_from_time(selected_range.min);
        let max_x = time_ranges_ui.x_from_time(selected_range.max);

        if let (Some(min_x), Some(max_x)) = (min_x, max_x) {
            let mut rect = Rect::from_x_y_ranges(min_x..=max_x, rect.y_range());

            // Make sure it is visible:
            if rect.width() < 2.0 {
                rect = Rect::from_x_y_ranges(
                    (rect.center().x - 1.0)..=(rect.center().x - 1.0),
                    rect.y_range(),
                );
            }

            let full_y_range = rect.top()..=time_area_painter.clip_rect().bottom();

            if is_active {
                let bg_color = selection_color.linear_multiply(transparent);
                time_area_painter.rect_filled(
                    Rect::from_x_y_ranges(rect.x_range(), full_y_range),
                    1.0,
                    bg_color,
                );
            }

            let main_color = if is_active {
                selection_color
            } else {
                selection_color.linear_multiply(transparent)
            };
            time_area_painter.rect_filled(rect, 1.0, main_color);

            if is_active {
                let range_text =
                    format_duration(time_control.time_type(), selected_range.length().abs());
                if !range_text.is_empty() {
                    let font_id = egui::TextStyle::Body.resolve(ui.style());
                    let text_color = ui.visuals().strong_text_color();
                    time_area_painter.text(
                        rect.left_center(),
                        Align2::LEFT_CENTER,
                        range_text,
                        font_id,
                        text_color,
                    );
                }
            }

            // Check for interaction:
            if let Some(pointer_pos) = pointer_pos {
                if rect.expand(interact_radius).contains(pointer_pos) {
                    let center_dist = (pointer_pos.x - rect.center().x).abs(); // make sure we can always move even small rects
                    let left_dist = (pointer_pos.x - min_x).abs();
                    let right_dist = (pointer_pos.x - max_x).abs();

                    let hovering_left =
                        left_dist < center_dist.min(right_dist).min(interact_radius);
                    let hovering_right =
                        !hovering_left && right_dist <= interact_radius.min(center_dist);
                    let hovering_move = !hovering_left
                        && !hovering_right
                        && (min_x <= pointer_pos.x && pointer_pos.x <= max_x);

                    let drag_started =
                        ui.input().pointer.any_pressed() && ui.input().pointer.primary_down();

                    if hovering_left {
                        ui.output().cursor_icon = CursorIcon::ResizeWest;
                        if drag_started {
                            ui.memory().set_dragged_id(left_edge_id);
                        }
                    } else if hovering_right {
                        ui.output().cursor_icon = CursorIcon::ResizeEast;
                        if drag_started {
                            ui.memory().set_dragged_id(right_edge_id);
                        }
                    } else if hovering_move {
                        ui.output().cursor_icon = CursorIcon::Move;
                        if drag_started {
                            ui.memory().set_dragged_id(move_id);
                        }
                    }

                    is_hovering_existing = hovering_left | hovering_right | hovering_move;
                }
            }
        }
    }

    // Start new selection?
    if let Some(pointer_pos) = pointer_pos {
        let is_anything_being_dragged = ui.memory().is_anything_being_dragged();
        if !is_hovering_existing
            && is_pointer_in_rect
            && !is_anything_being_dragged
            && ui.input().pointer.primary_down()
        {
            if let Some(time) = time_ranges_ui.time_from_x(pointer_pos.x) {
                time_control.set_time_selection(TimeRange::point(time));
                did_interact = true;
                ui.memory().set_dragged_id(right_edge_id);
            }
        }
    }

    // Resize/move (interact)
    if let Some(pointer_pos) = pointer_pos {
        if let Some(mut selected_range) = time_control.time_selection() {
            // Use "smart_aim" to find a natural length of the time interval
            let aim_radius = ui.input().aim_radius();
            use egui::emath::smart_aim::best_in_range_f64;

            if ui.memory().is_being_dragged(left_edge_id) {
                if let (Some(time_low), Some(time_high)) = (
                    time_ranges_ui.time_from_x(pointer_pos.x - aim_radius),
                    time_ranges_ui.time_from_x(pointer_pos.x + aim_radius),
                ) {
                    let low_length = selected_range.max - time_low;
                    let high_length = selected_range.max - time_high;
                    let best_length = TimeInt::from(best_in_range_f64(
                        low_length.to_f64(),
                        high_length.to_f64(),
                    ) as i64);

                    selected_range.min = selected_range.max - best_length;

                    if selected_range.min > selected_range.max {
                        std::mem::swap(&mut selected_range.min, &mut selected_range.max);
                        ui.memory().set_dragged_id(right_edge_id);
                    }

                    time_control.set_time_selection(selected_range);
                    did_interact = true;
                }
            }

            if ui.memory().is_being_dragged(right_edge_id) {
                if let (Some(time_low), Some(time_high)) = (
                    time_ranges_ui.time_from_x(pointer_pos.x - aim_radius),
                    time_ranges_ui.time_from_x(pointer_pos.x + aim_radius),
                ) {
                    let low_length = time_low - selected_range.min;
                    let high_length = time_high - selected_range.min;
                    let best_length = TimeInt::from(best_in_range_f64(
                        low_length.to_f64(),
                        high_length.to_f64(),
                    ) as i64);

                    selected_range.max = selected_range.min + best_length;

                    if selected_range.min > selected_range.max {
                        std::mem::swap(&mut selected_range.min, &mut selected_range.max);
                        ui.memory().set_dragged_id(left_edge_id);
                    }

                    time_control.set_time_selection(selected_range);
                    did_interact = true;
                }
            }

            if ui.memory().is_being_dragged(move_id) {
                (|| {
                    let min_x = time_ranges_ui.x_from_time(selected_range.min)?;
                    let max_x = time_ranges_ui.x_from_time(selected_range.max)?;

                    let min_x = min_x + ui.input().pointer.delta().x;
                    let max_x = max_x + ui.input().pointer.delta().x;

                    let min_time = time_ranges_ui.time_from_x(min_x)?;
                    let max_time = time_ranges_ui.time_from_x(max_x)?;

                    let mut new_range = TimeRange::new(min_time, max_time);

                    if egui::emath::almost_equal(
                        selected_range.length().to_f32(),
                        new_range.length().to_f32(),
                        1e-5,
                    ) {
                        // Avoid numerical inaccuracies: maintain length if very close
                        new_range.max = new_range.min + selected_range.length();
                    }

                    time_control.set_time_selection(new_range);
                    did_interact = true;
                    Some(())
                })();
            }
        }
    }

    if ui.memory().is_being_dragged(left_edge_id) {
        ui.output().cursor_icon = CursorIcon::ResizeWest;
    }
    if ui.memory().is_being_dragged(right_edge_id) {
        ui.output().cursor_icon = CursorIcon::ResizeEast;
    }
    if ui.memory().is_being_dragged(move_id) {
        ui.output().cursor_icon = CursorIcon::Move;
    }

    if did_interact {
        time_control.selection_active = true;
        if time_control.active_selection_type() == Some(TimeSelectionType::Loop) {
            time_control.set_looped(true);
        }
        if time_control.active_selection_type() == Some(TimeSelectionType::Filter) {
            time_control.pause();
        }
    }
}

/// Human-readable description of a duration
pub fn format_duration(time_typ: TimeType, duration: TimeInt) -> String {
    match time_typ {
        TimeType::Time => Duration::from(duration).to_string(),
        TimeType::Sequence => duration.to_i64().to_string(),
    }
}

fn time_marker_ui(
    time_ranges_ui: &TimeRangesUi,
    time_control: &mut TimeControl,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    time_line_rect: &Rect,
    bottom_y: f32,
) {
    // full_rect: full area.
    // time_line_rect: top part with the second ticks and time marker

    let pointer_pos = ui.input().pointer.hover_pos();
    let is_pointer_in_time_line_rect =
        pointer_pos.map_or(false, |pointer_pos| time_line_rect.contains(pointer_pos));

    // ------------------------------------------------

    let time_drag_id = ui.id().with("time_drag_id");

    let mut is_hovering = false;
    let mut is_dragging = ui.memory().is_being_dragged(time_drag_id);

    if is_pointer_in_time_line_rect {
        ui.output().cursor_icon = CursorIcon::ResizeHorizontal;
    }

    let mut is_anything_being_dragged = ui.memory().is_anything_being_dragged();

    // show current time as a line:
    if let Some(time) = time_control.time_int() {
        if let Some(x) = time_ranges_ui.x_from_time(time) {
            if let Some(pointer_pos) = pointer_pos {
                let line_rect = Rect::from_x_y_ranges(x..=x, time_line_rect.top()..=bottom_y);

                is_hovering = line_rect.distance_to_pos(pointer_pos)
                    <= ui.style().interaction.resize_grab_radius_side;

                if ui.input().pointer.any_pressed()
                    && ui.input().pointer.primary_down()
                    && is_hovering
                {
                    ui.memory().set_dragged_id(time_drag_id);
                    is_dragging = true; // avoid frame delay
                    is_anything_being_dragged = true;
                }
            }

            if is_dragging || (is_hovering && !is_anything_being_dragged) {
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
                pos2(x - 0.5 * w, time_line_rect.top()), // left top
                pos2(x + 0.5 * w, time_line_rect.top()), // right top
                pos2(x, time_line_rect.top() + w),       // bottom
            ];
            time_area_painter.add(egui::Shape::convex_polygon(
                triangle,
                stroke.color,
                egui::Stroke::none(),
            ));
            time_area_painter.vline(x, (time_line_rect.top() + w)..=bottom_y, stroke);
        }
    }

    // Show preview: "click here to view time here"
    if let Some(pointer_pos) = pointer_pos {
        if !is_hovering && !is_anything_being_dragged && is_pointer_in_time_line_rect {
            time_area_painter.vline(
                pointer_pos.x,
                time_line_rect.top()..=ui.max_rect().bottom(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
        }

        if is_dragging
            || (ui.input().pointer.primary_down()
                && is_pointer_in_time_line_rect
                && !is_anything_being_dragged)
        {
            if let Some(time) = time_ranges_ui.time_from_x(pointer_pos.x) {
                time_control.set_time(time);
                time_control.pause();
                ui.memory().set_dragged_id(time_drag_id);
            }
        }
    }
}

// ----------------------------------------------------------------------------

const MAX_GAP: f32 = 32.0;

/// How much space on side of the data in the default view.
const SIDE_MARGIN: f32 = MAX_GAP;

/// Sze of the gap between time segments.
fn gap_width(x_range: &RangeInclusive<f32>, segments: &[TimeRange]) -> f32 {
    let num_gaps = segments.len().saturating_sub(1);
    if num_gaps == 0 {
        // gap width doesn't matter when there are no gaps
        MAX_GAP
    } else {
        // shrink gaps if there are a lot of them
        let width = *x_range.end() - *x_range.start();
        (width / (4.0 * num_gaps as f32)).at_most(MAX_GAP)
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
    let time_spanned = time_source_axis.sum_time_lengths().to_f64() * factor as f64;

    // Leave some room on the margins:
    let time_margin = time_spanned * (SIDE_MARGIN / width.at_least(64.0)) as f64;
    let min = min - TimeInt::from(time_margin as i64);
    let time_spanned = time_spanned + 2.0 * time_margin;

    TimeView { min, time_spanned }
}

struct Segment {
    /// Matches [`Self::time`] (linear transform).
    x: RangeInclusive<f32>,

    /// Matches [`Self::x`] (linear transform).
    time: TimeRange,

    /// does NOT match any of the above. Instead this is a tight bound.
    tight_time: TimeRange,
}

/// Recreated each frame.
struct TimeRangesUi {
    /// The total x-range we are viewing
    x_range: RangeInclusive<f32>,

    time_view: TimeView,

    /// x ranges matched to time ranges
    segments: Vec<Segment>,

    /// x distance per time unit
    points_per_time: f32,
}

impl Default for TimeRangesUi {
    /// Safe, meaningless default
    fn default() -> Self {
        Self {
            x_range: 0.0..=1.0,
            time_view: TimeView {
                min: TimeInt::from(0),
                time_spanned: 1.0,
            },
            segments: vec![],
            points_per_time: 1.0,
        }
    }
}

impl TimeRangesUi {
    fn new(x_range: RangeInclusive<f32>, time_view: TimeView, segments: &[TimeRange]) -> Self {
        crate::profile_function!();

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
                let range_width = range.length().to_f32() * points_per_time;
                let right = left + range_width;
                let x_range = left..=right;
                left = right + gap_width;

                let tight_time = *range;

                // expand each span outwards a bit to make selection of outer data points easier.
                // Also gives zero-width segments some width!
                let expansion = gap_width / 3.0;
                let x_range = (*x_range.start() - expansion)..=(*x_range.end() + expansion);
                let time_expansion = TimeInt::from((expansion / points_per_time) as i64);
                let range = TimeRange::new(range.min - time_expansion, range.max + time_expansion);
                Segment {
                    x: x_range,
                    time: range,
                    tight_time,
                }
            })
            .collect();

        let mut slf = Self {
            x_range: x_range.clone(),
            time_view,
            segments: ranges,
            points_per_time,
        };

        if let Some(time_start_x) = slf.x_from_time(time_view.min) {
            // Now move things left/right to align `x_range` and `view_range`:
            let x_translate = *x_range.start() - time_start_x;
            for segment in &mut slf.segments {
                segment.x = (*segment.x.start() + x_translate)..=(*segment.x.end() + x_translate);
            }
        }

        slf
    }

    /// Make sure the time is not between ranges.
    fn snap_time(&self, value: TimeInt) -> TimeInt {
        for segment in &self.segments {
            if value < segment.time.min {
                return segment.time.min;
            } else if value <= segment.time.max {
                return value;
            }
        }
        value
    }

    // Make sure time doesn't get stuck between non-continuos regions:
    fn snap_time_control(&self, context: &mut ViewerContext) {
        if !context.time_control.is_playing() {
            return;
        }

        // Make sure time doesn't get stuck between non-continuos regions:
        if let Some(time) = context.time_control.time_int() {
            let time = self.snap_time(time);
            context.time_control.set_time(time);
        } else if let Some(selection) = context.time_control.time_selection() {
            let snapped_min = self.snap_time(selection.min);
            let snapped_max = self.snap_time(selection.max);

            let min_was_good = selection.min == snapped_min;
            let max_was_good = selection.max == snapped_max;

            if min_was_good || max_was_good {
                return;
            }

            // Keeping max works better when looping
            context.time_control.set_time_selection(TimeRange::new(
                snapped_max - selection.length(),
                snapped_max,
            ));
        }
    }

    fn x_from_time(&self, needle_time: TimeInt) -> Option<f32> {
        let first_segment = self.segments.first()?;
        let mut last_x = *first_segment.x.start();
        let mut last_time = first_segment.time.min;

        if needle_time <= last_time {
            // extrapolate:
            return Some(last_x - self.points_per_time * (last_time - needle_time).to_f32());
        }

        for segment in &self.segments {
            if needle_time < segment.time.min {
                let t = TimeRange::new(last_time, segment.time.min).inverse_lerp(needle_time);
                return Some(lerp(last_x..=*segment.x.start(), t));
            } else if needle_time <= segment.time.max {
                let t = segment.time.inverse_lerp(needle_time);
                return Some(lerp(segment.x.clone(), t));
            } else {
                last_x = *segment.x.end();
                last_time = segment.time.max;
            }
        }

        // extrapolate:
        Some(last_x + self.points_per_time * (needle_time - last_time).to_f32())
    }

    fn time_from_x(&self, needle_x: f32) -> Option<TimeInt> {
        let first_segment = self.segments.first()?;
        let mut last_x = *first_segment.x.start();
        let mut last_time = first_segment.time.min;

        if needle_x <= last_x {
            // extrapolate:
            return Some(
                last_time + TimeInt::from(((needle_x - last_x) / self.points_per_time) as i64),
            );
        }

        for segment in &self.segments {
            if needle_x < *segment.x.start() {
                let t = remap(needle_x, last_x..=*segment.x.start(), 0.0..=1.0);
                return Some(TimeRange::new(last_time, segment.time.min).lerp(t));
            } else if needle_x <= *segment.x.end() {
                let t = remap(needle_x, segment.x.clone(), 0.0..=1.0);
                return Some(segment.time.lerp(t));
            } else {
                last_x = *segment.x.end();
                last_time = segment.time.max;
            }
        }

        // extrapolate:
        Some(last_time + TimeInt::from(((needle_x - last_x) / self.points_per_time).round() as i64))
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

    /// How many egui points for each time unit?
    fn points_per_time(&self) -> Option<f32> {
        for segment in &self.segments {
            let dx = *segment.x.end() - *segment.x.start();
            let dt = segment.time.length().to_f32();
            if dx > 0.0 && dt > 0.0 {
                return Some(dx / dt);
            }
        }
        None
    }
}

fn paint_time_range_ticks(
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    rect: &Rect,
    time_type: TimeType,
    range: &TimeRange,
) {
    let font_id = egui::TextStyle::Body.resolve(ui.style());

    let (min, max) = (range.min.to_i64(), range.max.to_i64());

    let shapes = match time_type {
        TimeType::Time => {
            fn next_grid_tick_magnitude_ns(spacing_ns: i64) -> i64 {
                if spacing_ns <= 1_000_000_000 {
                    spacing_ns * 10 // up to 10 second ticks
                } else if spacing_ns == 10_000_000_000 {
                    spacing_ns * 6 // to the whole minute
                } else if spacing_ns == 60_000_000_000 {
                    spacing_ns * 10 // to ten minutes
                } else if spacing_ns == 600_000_000_000 {
                    spacing_ns * 6 // to an hour
                } else if spacing_ns < 24 * 60 * 60 * 1_000_000_000 {
                    spacing_ns * 24 // to a day
                } else {
                    spacing_ns * 10 // multiple of ten days
                }
            }

            fn grid_text_from_ns(ns: i64) -> String {
                let relative_ns = ns % 1_000_000_000;
                if relative_ns == 0 {
                    let time = Time::from_ns_since_epoch(ns);
                    if time.is_abolute_date() {
                        time.format_time("%H:%M:%S")
                    } else {
                        re_log_types::Duration::from_nanos(ns).to_string()
                    }
                } else {
                    // show relative to whole second:
                    let ms = relative_ns as f64 * 1e-6;
                    if relative_ns % 1_000_000 == 0 {
                        format!("{:+.0} ms", ms)
                    } else if relative_ns % 100_000 == 0 {
                        format!("{:+.1} ms", ms)
                    } else if relative_ns % 10_000 == 0 {
                        format!("{:+.2} ms", ms)
                    } else {
                        format!("{:+.3} ms", ms)
                    }
                }
            }

            paint_ticks(
                &ui.fonts(),
                ui.visuals().dark_mode,
                &font_id,
                rect,
                &ui.clip_rect(),
                (min, max), // ns
                1_000,
                next_grid_tick_magnitude_ns,
                grid_text_from_ns,
            )
        }
        TimeType::Sequence => {
            fn next_power_of_10(i: i64) -> i64 {
                i * 10
            }
            paint_ticks(
                &ui.fonts(),
                ui.visuals().dark_mode,
                &font_id,
                rect,
                &ui.clip_rect(),
                (min, max),
                1,
                next_power_of_10,
                |seq| format!("#{seq}"),
            )
        }
    };

    time_area_painter.extend(shapes);
}

#[allow(clippy::too_many_arguments)]
fn paint_ticks(
    fonts: &egui::epaint::Fonts,
    dark_mode: bool,
    font_id: &egui::FontId,
    canvas: &Rect,
    clip_rect: &Rect,
    (min_time, max_time): (i64, i64),
    min_grid_spacing_time: i64,
    next_time_step: fn(i64) -> i64,
    format_tick: fn(i64) -> String,
) -> Vec<egui::Shape> {
    let color_from_alpha = |alpha: f32| -> Color32 {
        if dark_mode {
            Rgba::from_white_alpha(alpha * alpha).into()
        } else {
            Rgba::from_black_alpha(alpha).into()
        }
    };

    let x_from_time = |time: i64| -> f32 {
        let t = time.saturating_sub(min_time) as f32 / max_time.saturating_sub(min_time) as f32;
        lerp(canvas.x_range(), t)
    };

    let visible_rect = clip_rect.intersect(*canvas);
    let mut shapes = vec![];

    if !visible_rect.is_positive() {
        return shapes;
    }

    let width_time = max_time - min_time;
    let points_per_time = canvas.width() / width_time as f32;
    let minimum_small_line_spacing = 4.0;
    let expected_text_width = 60.0;

    let line_color_from_spacing = |spacing_time: i64| -> Color32 {
        let next_tick_magnitude = next_time_step(spacing_time) / spacing_time; // usually 10, but could be 6 or 24 for time
        let alpha = remap_clamp(
            spacing_time as f32 * points_per_time,
            minimum_small_line_spacing..=(next_tick_magnitude as f32 * minimum_small_line_spacing),
            0.0..=0.6,
        );
        color_from_alpha(alpha)
    };

    let text_color_from_spacing = |spacing_time: i64| -> Color32 {
        let alpha = remap_clamp(
            spacing_time as f32 * points_per_time,
            expected_text_width..=(3.0 * expected_text_width),
            0.0..=1.0,
        );
        color_from_alpha(alpha)
    };

    let max_small_lines = canvas.width() / minimum_small_line_spacing;
    let mut small_spacing_time = min_grid_spacing_time;
    while width_time as f32 / (small_spacing_time as f32) > max_small_lines {
        small_spacing_time = next_time_step(small_spacing_time);
    }
    let medium_spacing_time = next_time_step(small_spacing_time);
    let big_spacing_time = next_time_step(medium_spacing_time);

    // We fade in lines as we zoom in:
    let big_line_color = line_color_from_spacing(big_spacing_time);
    let medium_line_color = line_color_from_spacing(medium_spacing_time);
    let small_line_color = line_color_from_spacing(small_spacing_time);

    let big_text_color = text_color_from_spacing(big_spacing_time);
    let medium_text_color = text_color_from_spacing(medium_spacing_time);
    let small_text_color = text_color_from_spacing(small_spacing_time);

    let mut current_time = min_time / small_spacing_time * small_spacing_time; // TODO(emilk): start at visible_rect.left()
    while current_time <= max_time {
        let line_x = x_from_time(current_time);

        if visible_rect.min.x <= line_x && line_x <= visible_rect.max.x {
            let medium_line = current_time % medium_spacing_time == 0;
            let big_line = current_time % big_spacing_time == 0;

            let (line_color, text_color) = if big_line {
                (big_line_color, big_text_color)
            } else if medium_line {
                (medium_line_color, medium_text_color)
            } else {
                (small_line_color, small_text_color)
            };

            let top = if current_time % 1_000_000_000 == 0 {
                // TODO(emilk): for sequences (non-nanoseconds)
                canvas.top() // full second
            } else {
                lerp(canvas.y_range(), 0.75)
            };

            shapes.push(egui::Shape::line_segment(
                [pos2(line_x, top), pos2(line_x, canvas.max.y)],
                Stroke::new(1.0, line_color),
            ));

            if text_color != Color32::TRANSPARENT {
                let text = format_tick(current_time);
                let text_x = line_x + 4.0;

                // Text at top:
                shapes.push(egui::Shape::text(
                    fonts,
                    pos2(text_x, canvas.min.y),
                    Align2::LEFT_TOP,
                    &text,
                    font_id.clone(),
                    text_color,
                ));
            }
        }

        current_time += small_spacing_time;
    }

    shapes
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

        let y = if max_y <= min_y {
            center_y
        } else {
            let mut best_free_y = f32::INFINITY;
            let mut best_colliding_y = center_y;
            let mut best_colliding_d2 = 0.0;

            let step_size = 2.0; // unit: points

            for y_offset in 0..=((max_y - min_y) / step_size).round() as i32 {
                let y = min_y + step_size * y_offset as f32;
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

            if best_free_y.is_finite() {
                best_free_y
            } else {
                best_colliding_y
            }
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
