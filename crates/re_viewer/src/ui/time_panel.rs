use std::{
    collections::{BTreeMap, BTreeSet},
    ops::RangeInclusive,
};

use egui::{
    lerp, pos2, remap, remap_clamp, show_tooltip_at_pointer, Align2, Color32, CursorIcon, Id,
    NumExt, PointerButton, Pos2, Rect, Rgba, Shape, Stroke, Vec2,
};

use itertools::Itertools;
use re_data_store::{InstanceId, ObjectTree};
use re_log_types::{
    DataPath, Duration, MsgId, ObjPathComp, Time, TimeInt, TimeRange, TimeRangeF, TimeReal,
    TimeType,
};

use crate::{
    misc::time_control::Looping, time_axis::TimelineAxis, Selection, TimeControl, TimeView,
    ViewerContext,
};

use super::{data_ui::DataUi, Blueprint};

/// A panel that shows objects to the left, time on the top.
///
/// This includes the timeline controls and streams view.
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
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        egui_ctx: &egui::Context,
    ) {
        let x_margin = 16.0;
        let y_margin = 8.0;
        let top_bar_height = 28.0;

        let mut panel_frame = egui::Frame {
            fill: ctx.re_ui.design_tokens.bottom_bar_color,
            inner_margin: egui::style::Margin::symmetric(x_margin, y_margin),
            ..ctx.re_ui.panel_frame()
        };

        if blueprint.time_panel_expanded {
            // Since we use scroll bars we want to fill the whole vertical space downwards:
            panel_frame.inner_margin.bottom = 0.0;

            // Similarly, let the data get close to the right edge:
            panel_frame.inner_margin.right = 0.0;
        }

        let collapsed = egui::TopBottomPanel::bottom("time_panel_collapsed")
            .resizable(false)
            .frame(panel_frame)
            .default_height(16.0);
        let expanded = egui::TopBottomPanel::bottom("time_panel_expanded")
            .resizable(true)
            .frame(panel_frame)
            .min_height(150.0)
            .default_height(250.0);

        egui::TopBottomPanel::show_animated_between(
            egui_ctx,
            blueprint.time_panel_expanded,
            collapsed,
            expanded,
            |ui: &mut egui::Ui, expansion: f32| {
                if expansion < 1.0 {
                    // Collapsed or animating
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size = Vec2::splat(top_bar_height);
                        ui.visuals_mut().button_frame = true;
                        self.collapsed_ui(ctx, ui);
                    });
                } else {
                    // Expanded:
                    ui.vertical(|ui| {
                        // Add back the margin we removed from the panel:
                        let mut top_rop_frame = egui::Frame::default();
                        top_rop_frame.inner_margin.right = x_margin;
                        top_rop_frame.inner_margin.bottom = y_margin;
                        let rop_row_rect = top_rop_frame
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().interact_size = Vec2::splat(top_bar_height);
                                    ui.visuals_mut().button_frame = true;
                                    top_row_ui(ctx, ui);
                                });
                            })
                            .response
                            .rect;

                        // Draw separator between top bar and the rest:
                        ui.painter().hline(
                            0.0..=rop_row_rect.right(),
                            rop_row_rect.bottom(),
                            ui.visuals().widgets.noninteractive.bg_stroke,
                        );

                        ui.spacing_mut().scroll_bar_outer_margin = 4.0; // needed, because we have no panel margin on the right side.
                        self.expanded_ui(ctx, ui);
                    });
                }
            },
        );
    }

    #[allow(clippy::unused_self)]
    fn collapsed_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ctx.rec_cfg
            .time_ctrl
            .play_pause_ui(ctx.re_ui, ctx.log_db.times_per_timeline(), ui);

        ui.separator();

        ctx.rec_cfg
            .time_ctrl
            .timeline_selector_ui(ctx.log_db.times_per_timeline(), ui);

        ui.separator();

        {
            let time_range_width = 400.0;
            let mut time_range_rect = ui.available_rect_before_wrap();
            time_range_rect.max.x = f32::min(
                time_range_rect.max.x - 220.0, // save space for current time and help button,
                time_range_rect.min.x + time_range_width,
            );

            if time_range_rect.width() > 50.0 {
                let time_ranges_ui =
                    initialize_time_ranges_ui(ctx, time_range_rect.x_range(), None);
                time_ranges_ui.snap_time_control(ctx);

                let painter = ui.painter_at(time_range_rect.expand(4.0));
                painter.hline(
                    time_range_rect.x_range(),
                    time_range_rect.center().y,
                    ui.visuals().widgets.inactive.fg_stroke,
                );
                time_marker_ui(
                    &time_ranges_ui,
                    &mut ctx.rec_cfg.time_ctrl,
                    ui,
                    &painter,
                    &time_range_rect,
                );

                ui.allocate_rect(time_range_rect, egui::Sense::hover());
            }
        }

        current_time_ui(ctx, ui);
    }

    fn expanded_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        //               |timeline            |
        // ------------------------------------
        // tree          |streams             |
        //               |  . .   .    ...    |
        //               |             ...  . |

        self.next_col_right = ui.min_rect().left(); // next_col_right will expand during the call

        let time_x_left = ui.min_rect().left() + self.prev_col_width + ui.spacing().item_spacing.x;

        // Where the time will be shown.
        let time_bg_x_range = time_x_left..=ui.max_rect().right();
        let time_fg_x_range = {
            // Painting to the right of the scroll bar (if any) looks bad:
            let right = ui.max_rect().right() - ui.spacing_mut().scroll_bar_outer_margin;
            time_x_left..=right
        };

        let side_margin = 26.0; // chosen so that the scroll bar looks approximately centered in the default gap
        self.time_ranges_ui = initialize_time_ranges_ui(
            ctx,
            (*time_fg_x_range.start() + side_margin)..=(*time_fg_x_range.end() - side_margin),
            ctx.rec_cfg.time_ctrl.time_view(),
        );
        let full_y_range = ui.min_rect().bottom()..=ui.max_rect().bottom();

        let timeline_rect = {
            let top = ui.min_rect().bottom();

            let size = egui::vec2(self.prev_col_width, 28.0);
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.set_min_size(size);
                ui.add_space(4.0); // hack to vertically center the text
                ui.strong("Streams");
            })
            .response
            .on_hover_text(
                "A hierarchial view of the paths used during logging.\n\
                        \n\
                        On the right you can see when there was a log event for a stream.",
            );

            let bottom = ui.min_rect().bottom();
            Rect::from_x_y_ranges(time_fg_x_range.clone(), top..=bottom)
        };

        let streams_rect = Rect::from_x_y_ranges(
            time_fg_x_range.clone(),
            timeline_rect.bottom()..=ui.max_rect().bottom(),
        );

        // includes the timeline and streams areas.
        let time_bg_area_rect = Rect::from_x_y_ranges(time_bg_x_range, full_y_range.clone());
        let time_fg_area_rect =
            Rect::from_x_y_ranges(time_fg_x_range.clone(), full_y_range.clone());
        let time_bg_area_painter = ui.painter().with_clip_rect(time_bg_area_rect);
        let time_area_painter = ui.painter().with_clip_rect(time_fg_area_rect);

        ui.painter().hline(
            0.0..=ui.max_rect().right(),
            timeline_rect.bottom(),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        paint_time_ranges_and_ticks(
            &self.time_ranges_ui,
            ui,
            &time_area_painter,
            timeline_rect.top()..=timeline_rect.bottom(),
            ctx.rec_cfg.time_ctrl.time_type(),
        );
        paint_time_ranges_gaps(
            &self.time_ranges_ui,
            ui,
            &time_bg_area_painter,
            full_y_range.clone(),
        );
        loop_selection_ui(
            &self.time_ranges_ui,
            &mut ctx.rec_cfg.time_ctrl,
            ui,
            &time_bg_area_painter,
            &timeline_rect,
        );
        let time_area_response = interact_with_streams_rect(
            &self.time_ranges_ui,
            &mut ctx.rec_cfg.time_ctrl,
            ui,
            &time_bg_area_rect,
            &streams_rect,
        );

        // Don't draw on top of the time ticks
        let lower_time_area_painter = ui.painter().with_clip_rect(Rect::from_x_y_ranges(
            time_fg_x_range,
            ui.min_rect().bottom()..=ui.max_rect().bottom(),
        ));

        // All the object rows:
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            // We turn off `drag_to_scroll` so that the `ScrollArea` don't steal input from
            // the earlier `interact_with_time_area`.
            // We implement drag-to-scroll manually instead!
            .drag_to_scroll(false)
            .show(ui, |ui| {
                crate::profile_scope!("tree_ui");
                if time_area_response.dragged_by(PointerButton::Primary) {
                    ui.scroll_with_delta(Vec2::Y * time_area_response.drag_delta().y);
                }
                self.tree_ui(ctx, &time_area_response, &lower_time_area_painter, ui);
            });

        {
            // Paint a line between the stream names on the left
            // and the data on the right:
            ui.painter().vline(
                time_x_left,
                full_y_range,
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
        }

        // Put time-marker on top and last, so that you can always drag it
        time_marker_ui(
            &self.time_ranges_ui,
            &mut ctx.rec_cfg.time_ctrl,
            ui,
            &time_area_painter,
            &timeline_rect,
        );

        self.time_ranges_ui.snap_time_control(ctx);

        // remember where to show the time for next frame:
        self.prev_col_width = self.next_col_right - ui.min_rect().left();
    }

    fn tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        ui: &mut egui::Ui,
    ) {
        self.show_children(
            ctx,
            time_area_response,
            time_area_painter,
            &ctx.log_db.obj_db.tree,
            ui,
        );
    }

    fn show_tree(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        // the parent path of the name component
        last_component: &ObjPathComp,
        tree: &ObjectTree,
        ui: &mut egui::Ui,
    ) {
        if !tree
            .prefix_times
            .has_timeline(ctx.rec_cfg.time_ctrl.timeline())
        {
            return; // ignore objects that have no data for the current timeline
        }

        // The last part of the the path component
        let text = if tree.is_leaf() {
            last_component.to_string()
        } else {
            format!("{}/", last_component) // show we have children with a /
        };

        let collapsing_header_id = ui.make_persistent_id(&tree.path);
        let default_open = tree.path.len() <= 1 && !tree.is_leaf();
        let (_collapsing_button_response, custom_header_response, body_returned) =
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                collapsing_header_id,
                default_open,
            )
            .show_header(ui, |ui| ctx.obj_path_button_to(ui, None, &tree.path, text))
            .body(|ui| {
                self.show_children(ctx, time_area_response, time_area_painter, tree, ui);
            });

        let is_closed = body_returned.is_none();
        let response = custom_header_response.response;
        let response_rect = response.rect;
        self.next_col_right = self.next_col_right.max(response_rect.right());

        let full_width_rect = Rect::from_x_y_ranges(
            response_rect.left()..=ui.max_rect().right(),
            response_rect.y_range(),
        );

        let is_visible = ui.is_rect_visible(full_width_rect);

        if is_visible {
            // paint hline guide:
            let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            stroke.color = stroke.color.linear_multiply(0.5);
            let left = response_rect.left() + ui.spacing().indent;
            let y = response_rect.bottom() + ui.spacing().item_spacing.y * 0.5;
            ui.painter().hline(left..=ui.max_rect().right(), y, stroke);
        }

        // ----------------------------------------------

        // show the data in the time area:

        if is_visible && is_closed {
            if let Some(messages_over_time) =
                tree.prefix_times.get(ctx.rec_cfg.time_ctrl.timeline())
            {
                show_data_over_time(
                    ctx,
                    time_area_response,
                    time_area_painter,
                    ui,
                    messages_over_time,
                    full_width_rect,
                    &self.time_ranges_ui,
                    Some(Selection::Instance(
                        None,
                        InstanceId {
                            obj_path: tree.path.clone(),
                            instance_index: None,
                        },
                    )),
                );
            }
        }
    }

    fn show_children(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        tree: &ObjectTree,
        ui: &mut egui::Ui,
    ) {
        for (last_component, child) in &tree.children {
            self.show_tree(
                ctx,
                time_area_response,
                time_area_painter,
                last_component,
                child,
                ui,
            );
        }

        // If this is an object:
        if !tree.fields.is_empty() {
            let indent = ui.spacing().indent;

            for (field_name, data) in &tree.fields {
                if !data.times.contains_key(ctx.rec_cfg.time_ctrl.timeline()) {
                    continue; // ignore fields that have no data for the current timeline
                }

                let data_path = DataPath::new_any(tree.path.clone(), *field_name);

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
                        ctx.data_path_button_to(
                            ui,
                            super::format_field_or_component_name(field_name),
                            &data_path,
                        );
                    })
                    .response;

                self.next_col_right = self.next_col_right.max(response.rect.right());

                let full_width_rect = Rect::from_x_y_ranges(
                    response.rect.left()..=ui.max_rect().right(),
                    response.rect.y_range(),
                );
                let is_visible = ui.is_rect_visible(full_width_rect);

                if is_visible {
                    // paint hline guide:
                    let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
                    stroke.color = stroke.color.linear_multiply(0.5);
                    let left = response.rect.left() + ui.spacing().indent;
                    let y = response.rect.bottom() + ui.spacing().item_spacing.y * 0.5;
                    ui.painter().hline(left..=ui.max_rect().right(), y, stroke);
                }

                if is_visible {
                    response.on_hover_ui(|ui| {
                        ui.strong("Data");
                        ui.label(data_path.to_string());
                        let summary = data.summary();
                        if !summary.is_empty() {
                            ui.label(summary);
                        }
                    });
                }

                // show the data in the time area:

                if is_visible {
                    if let Some(messages_over_time) =
                        data.times.get(ctx.rec_cfg.time_ctrl.timeline())
                    {
                        show_data_over_time(
                            ctx,
                            time_area_response,
                            time_area_painter,
                            ui,
                            messages_over_time,
                            full_width_rect,
                            &self.time_ranges_ui,
                            Some(Selection::DataPath(data_path)),
                        );
                    }
                }
            }
        }
    }
}

fn top_row_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    ctx.rec_cfg
        .time_ctrl
        .play_pause_ui(ctx.re_ui, ctx.log_db.times_per_timeline(), ui);

    ui.separator();

    ctx.rec_cfg
        .time_ctrl
        .timeline_selector_ui(ctx.log_db.times_per_timeline(), ui);

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        help_button(ui);
        ui.centered_and_justified(|ui| {
            current_time_ui(ctx, ui);
        });
    });
}

fn help_button(ui: &mut egui::Ui) {
    crate::misc::help_hover_button(ui).on_hover_text(
        "\
        In the top row you can drag to move the time, or shift-drag to select a loop region.\n\
        \n\
        Drag main area to pan.\n\
        Zoom: Ctrl/cmd + scroll, or drag up/down with secondary mouse button.\n\
        Double-click to reset view.\n\
        \n\
        Press spacebar to play/pause.",
    );
}

fn current_time_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    if let Some(time_int) = ctx.rec_cfg.time_ctrl.time_int() {
        let time_type = ctx.rec_cfg.time_ctrl.time_type();
        ui.monospace(time_type.format(time_int));
    }
}

#[allow(clippy::too_many_arguments)]
fn show_data_over_time(
    ctx: &mut ViewerContext<'_>,
    time_area_response: &egui::Response,
    time_area_painter: &egui::Painter,
    ui: &mut egui::Ui,
    messages_over_time: &BTreeMap<TimeInt, BTreeSet<MsgId>>,
    full_width_rect: Rect,
    time_ranges_ui: &TimeRangesUi,
    select_on_click: Option<Selection>,
) {
    crate::profile_function!();

    // TODO(andreas): Should pass through underlying instance id and be clever about selection vs hover state.
    let is_selected = if let Some(select_on_click) = select_on_click.as_ref() {
        ctx.selection().iter().contains(select_on_click)
    } else {
        false
    };

    // painting each data point as a separate circle is slow (too many circles!)
    // so we join time points that are close together.
    let points_per_time = time_ranges_ui.points_per_time().unwrap_or(f32::INFINITY);
    let max_stretch_length_in_time = 1.0 / points_per_time as f64; // TODO(emilk)

    let pointer_pos = ui.input().pointer.hover_pos();

    let hovered_color = ui.visuals().widgets.hovered.text_color();
    let inactive_color = if is_selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals()
            .widgets
            .inactive
            .text_color()
            .linear_multiply(0.75)
    };

    struct Stretch<'a> {
        start_x: f32,
        start_time: TimeInt,
        stop_time: TimeInt,
        selected: bool,
        msg_ids: Vec<&'a BTreeSet<MsgId>>,
    }

    let mut shapes = vec![];
    let mut scatter = BallScatterer::default();
    let mut hovered_messages = vec![];
    let mut hovered_time = None;

    let mut paint_stretch = |stretch: &Stretch<'_>| {
        let stop_x = time_ranges_ui
            .x_from_time(stretch.stop_time.into())
            .unwrap_or(stretch.start_x);

        let num_messages: usize = stretch.msg_ids.iter().map(|l| l.len()).sum();
        let radius = 2.5 * (1.0 + 0.5 * (num_messages as f32).log10());
        let radius = radius.at_most(full_width_rect.height() / 3.0);
        debug_assert!(radius.is_finite());

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

        if is_hovered {
            hovered_messages.extend(stretch.msg_ids.iter().copied().flatten().copied());
            hovered_time.get_or_insert(stretch.start_time);
        }
    };

    let mut stretch: Option<Stretch<'_>> = None;

    let margin = 5.0;
    let visible_time_range = TimeRange {
        min: time_ranges_ui
            .time_from_x(time_area_painter.clip_rect().left() - margin)
            .map_or(TimeInt::MIN, |tf| tf.floor()),

        max: time_ranges_ui
            .time_from_x(time_area_painter.clip_rect().right() + margin)
            .map_or(TimeInt::MAX, |tf| tf.ceil()),
    };

    let selected_time_range = ctx.rec_cfg.time_ctrl.active_loop_selection();

    for (&time, msg_ids) in
        messages_over_time.range(visible_time_range.min..=visible_time_range.max)
    {
        if msg_ids.is_empty() {
            continue;
        }
        let time_real = TimeReal::from(time);

        let selected = selected_time_range.map_or(true, |range| range.contains(time_real));

        if let Some(current_stretch) = &mut stretch {
            if current_stretch.selected == selected
                && (time - current_stretch.start_time).as_f64() < max_stretch_length_in_time
            {
                // extend:
                current_stretch.stop_time = time;
                current_stretch.msg_ids.push(msg_ids);
            } else {
                // stop the previous…
                paint_stretch(current_stretch);

                stretch = None;
            }
        }

        if stretch.is_none() {
            if let Some(x) = time_ranges_ui.x_from_time(time_real) {
                stretch = Some(Stretch {
                    start_x: x,
                    start_time: time,
                    stop_time: time,
                    selected,
                    msg_ids: vec![msg_ids],
                });
            }
        }
    }

    if let Some(stretch) = stretch {
        paint_stretch(&stretch);
    }

    time_area_painter.extend(shapes);

    if !hovered_messages.is_empty() {
        if time_area_response.clicked_by(egui::PointerButton::Primary) {
            if let Some(select_on_click) = select_on_click {
                ctx.set_single_selection(select_on_click);
            } else {
                ctx.selection_state_mut().clear_current();
            }

            if let Some(hovered_time) = hovered_time {
                ctx.rec_cfg.time_ctrl.set_time(hovered_time);
                ctx.rec_cfg.time_ctrl.pause();
            }
        } else if !ui.ctx().memory().is_anything_being_dragged() {
            show_msg_ids_tooltip(ctx, ui.ctx(), &hovered_messages);
        }
    }
}

fn show_msg_ids_tooltip(ctx: &mut ViewerContext<'_>, egui_ctx: &egui::Context, msg_ids: &[MsgId]) {
    show_tooltip_at_pointer(egui_ctx, Id::new("data_tooltip"), |ui| {
        // TODO(emilk): show as a table?
        if msg_ids.len() == 1 {
            let msg_id = msg_ids[0];
            if let Some(msg) = ctx.log_db.get_log_msg(&msg_id) {
                ui.push_id(msg_id, |ui| {
                    ui.group(|ui| {
                        msg.data_ui(ctx, ui, crate::Preview::Small);
                    });
                });
            }
        } else {
            ui.label(format!(
                "{} messages",
                re_format::format_number(msg_ids.len())
            ));
        }
    });
}

// ----------------------------------------------------------------------------

fn initialize_time_ranges_ui(
    ctx: &mut ViewerContext<'_>,
    time_x_range: RangeInclusive<f32>,
    time_view: Option<TimeView>,
) -> TimeRangesUi {
    crate::profile_function!();
    if let Some(times) = ctx
        .log_db
        .times_per_timeline()
        .get(ctx.rec_cfg.time_ctrl.timeline())
    {
        let timeline_axis = TimelineAxis::new(ctx.rec_cfg.time_ctrl.time_type(), times);
        let time_view = time_view.unwrap_or_else(|| view_everything(&time_x_range, &timeline_axis));

        TimeRangesUi::new(time_x_range, time_view, &timeline_axis.ranges)
    } else {
        Default::default()
    }
}

fn paint_time_ranges_and_ticks(
    time_ranges_ui: &TimeRangesUi,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    line_y_range: RangeInclusive<f32>,
    time_type: TimeType,
) {
    let clip_rect = ui.clip_rect();

    for segment in &time_ranges_ui.segments {
        let mut x_range = segment.x.clone();
        let mut time_range = segment.time;

        // Cull:
        if *x_range.end() < clip_rect.left() {
            continue;
        }
        if clip_rect.right() < *x_range.start() {
            continue;
        }

        // Clamp segment to the visible portion to save CPU when zoomed in:
        let left_t = egui::emath::inverse_lerp(x_range.clone(), clip_rect.left()).unwrap_or(0.5);
        if 0.0 < left_t && left_t < 1.0 {
            x_range = clip_rect.left()..=*x_range.end();
            time_range = TimeRangeF::new(time_range.lerp(left_t), time_range.max);
        }
        let right_t = egui::emath::inverse_lerp(x_range.clone(), clip_rect.right()).unwrap_or(0.5);
        if 0.0 < right_t && right_t < 1.0 {
            x_range = *x_range.start()..=clip_rect.right();
            time_range = TimeRangeF::new(time_range.min, time_range.lerp(right_t));
        }

        let rect = Rect::from_x_y_ranges(x_range, line_y_range.clone());
        time_area_painter
            .with_clip_rect(rect)
            .extend(paint_time_range_ticks(ui, &rect, time_type, &time_range));
    }
}

/// Visually separate the different time segments
fn paint_time_ranges_gaps(
    time_ranges_ui: &TimeRangesUi,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    y_range: RangeInclusive<f32>,
) {
    crate::profile_function!();

    // For each gap we are painting this:
    //
    //             zig width
    //             |
    //            <->
    //    \         /  ^
    //     \       /   | zig height
    //      \     /    v
    //      /     \
    //     /       \
    //    /         \
    //    \         /
    //     \       /
    //      \     /
    //      /     \
    //     /       \
    //    /         \
    //
    //    <--------->
    //     gap width
    //
    // Filled with black, plus a stroke.

    use itertools::Itertools as _;

    let top = *y_range.start();
    let bottom = *y_range.end();

    let fill_color = Color32::BLACK;
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;

    let paint_time_gap = |gap_left: f32, gap_right: f32| {
        let gap_width = gap_right - gap_left;
        let zig_width = 4.0_f32.at_most(gap_width / 3.0).at_least(1.0);
        let zig_height = zig_width;

        let mut y = top;
        let mut row = 0; // 0 = start wide, 1 = start narrow

        let mut mesh = egui::Mesh::default();
        let mut left_line_strip = vec![];
        let mut right_line_strip = vec![];

        while y - zig_height <= bottom {
            let (left, right) = if row % 2 == 0 {
                // full width
                (gap_left, gap_right)
            } else {
                // contracted
                (gap_left + zig_width, gap_right - zig_width)
            };

            let left_pos = pos2(left, y);
            let right_pos = pos2(right, y);

            if !mesh.is_empty() {
                let next_left_vidx = mesh.vertices.len() as u32;
                let next_right_vidx = next_left_vidx + 1;
                let prev_left_vidx = next_left_vidx - 2;
                let prev_right_vidx = next_right_vidx - 2;

                mesh.add_triangle(prev_left_vidx, next_left_vidx, prev_right_vidx);
                mesh.add_triangle(next_left_vidx, prev_right_vidx, next_right_vidx);
            }

            mesh.colored_vertex(left_pos, fill_color);
            mesh.colored_vertex(right_pos, fill_color);

            left_line_strip.push(left_pos);
            right_line_strip.push(right_pos);

            y += zig_height;
            row += 1;
        }

        painter.add(Shape::Mesh(mesh));
        painter.add(Shape::line(left_line_strip, stroke));
        painter.add(Shape::line(right_line_strip, stroke));
    };

    let zig_zag_first_and_last_edges = true;

    if let Some(segment) = time_ranges_ui.segments.first() {
        let gap_edge = *segment.x.start();

        if zig_zag_first_and_last_edges {
            // Left side of first segment - paint as a very wide gap that we only see the right side of
            paint_time_gap(gap_edge - 100_000.0, gap_edge);
        } else {
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge - 100_000.0, top), pos2(gap_edge, bottom)),
                0.0,
                fill_color,
            );
            painter.vline(gap_edge, y_range.clone(), stroke);
        }
    }

    for (a, b) in time_ranges_ui.segments.iter().tuple_windows() {
        paint_time_gap(*a.x.end(), *b.x.start());
    }

    if let Some(segment) = time_ranges_ui.segments.last() {
        let gap_edge = *segment.x.end();
        if zig_zag_first_and_last_edges {
            // Right side of last segment - paint as a very wide gap that we only see the left side of
            paint_time_gap(gap_edge, gap_edge + 100_000.0);
        } else {
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge, top), pos2(gap_edge + 100_000.0, bottom)),
                0.0,
                fill_color,
            );
            painter.vline(gap_edge, y_range, stroke);
        }
    }
}

/// Returns a scroll delta
#[must_use]
fn interact_with_streams_rect(
    time_ranges_ui: &TimeRangesUi,
    time_ctrl: &mut TimeControl,
    ui: &mut egui::Ui,
    full_rect: &Rect,
    streams_rect: &Rect,
) -> egui::Response {
    let pointer_pos = ui.input().pointer.hover_pos();

    let mut delta_x = 0.0;
    let mut zoom_factor = 1.0;

    // Check for zoom/pan inputs (via e.g. horizontal scrolling) on the entire
    // time area rectangle, including the timeline rect.
    let full_rect_hovered =
        pointer_pos.map_or(false, |pointer_pos| full_rect.contains(pointer_pos));
    if full_rect_hovered {
        delta_x += ui.input().scroll_delta.x;
        zoom_factor *= ui.input().zoom_delta_2d().x;
    }

    // We only check for drags in the streams rect,
    // because drags in the timeline rect should move the time
    // (or create loop sections).
    let response = ui.interact(
        *streams_rect,
        ui.id().with("time_area_interact"),
        egui::Sense::click_and_drag(),
    );
    if response.dragged_by(PointerButton::Primary) {
        delta_x += response.drag_delta().x;
        ui.output().cursor_icon = CursorIcon::AllScroll;
    }
    if response.dragged_by(PointerButton::Secondary) {
        zoom_factor *= (response.drag_delta().y * 0.01).exp();
    }

    if delta_x != 0.0 {
        if let Some(new_view_range) = time_ranges_ui.pan(-delta_x) {
            time_ctrl.set_time_view(new_view_range);
        }
    }

    if zoom_factor != 1.0 {
        if let Some(pointer_pos) = pointer_pos {
            if let Some(new_view_range) = time_ranges_ui.zoom_at(pointer_pos.x, zoom_factor) {
                time_ctrl.set_time_view(new_view_range);
            }
        }
    }

    if response.double_clicked() {
        time_ctrl.reset_time_view();
    }

    response
}

fn initial_time_selection(
    time_ranges_ui: &TimeRangesUi,
    time_type: TimeType,
) -> Option<TimeRangeF> {
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
                            return Some(TimeRangeF::new(range.min, range.min + one_sec));
                        }
                    }
                    TimeType::Sequence => {
                        return Some(TimeRangeF::new(
                            range.min,
                            TimeReal::from(range.min)
                                + TimeReal::from((range.max - range.min).as_f64() / 2.0),
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
        Some(TimeRangeF::new(
            ranges[0].tight_time.min,
            ranges[end].tight_time.max,
        ))
    }
}

fn loop_selection_ui(
    time_ranges_ui: &TimeRangesUi,
    time_ctrl: &mut TimeControl,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    timeline_rect: &Rect,
) {
    if time_ctrl.loop_selection().is_none() {
        // Helpfully select a time slice so that there always is a selection.
        // This helps new users ("what is that?").
        if let Some(selection) = initial_time_selection(time_ranges_ui, time_ctrl.time_type()) {
            time_ctrl.set_loop_selection(selection);
        }
    }

    if time_ctrl.loop_selection().is_none() && time_ctrl.looping == Looping::Selection {
        time_ctrl.looping = Looping::Off;
    }

    // TODO(emilk): click to toggle on/off
    // when off, you cannot modify, just drag out a new one.

    let selection_color = re_ui::ReUi::loop_selection_color();

    let is_active = time_ctrl.looping == Looping::Selection;

    let pointer_pos = ui.input().pointer.hover_pos();
    let is_pointer_in_timeline =
        pointer_pos.map_or(false, |pointer_pos| timeline_rect.contains(pointer_pos));

    let left_edge_id = ui.id().with("selection_left_edge");
    let right_edge_id = ui.id().with("selection_right_edge");
    let middle_id = ui.id().with("selection_move");

    let interact_radius = ui.style().interaction.resize_grab_radius_side;

    let transparent = if ui.visuals().dark_mode { 0.06 } else { 0.3 };

    // Paint existing selection and detect drag starting and hovering:
    if let Some(mut selected_range) = time_ctrl.loop_selection() {
        let min_x = time_ranges_ui.x_from_time(selected_range.min);
        let max_x = time_ranges_ui.x_from_time(selected_range.max);

        if let (Some(min_x), Some(max_x)) = (min_x, max_x) {
            let mut rect = Rect::from_x_y_ranges(min_x..=max_x, timeline_rect.y_range());

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

            if is_active && !selected_range.is_empty() {
                paint_range_text(time_ctrl, selected_range, ui, time_area_painter, rect);
            }

            // Check for interaction:
            if let Some(pointer_pos) = pointer_pos {
                let left_edge_rect =
                    Rect::from_x_y_ranges(rect.left()..=rect.left(), rect.y_range())
                        .expand(interact_radius);

                let right_edge_rect =
                    Rect::from_x_y_ranges(rect.right()..=rect.right(), rect.y_range())
                        .expand(interact_radius);

                // Check middle first, so that the edges "wins" (are on top)
                let middle_response = ui
                    .interact(rect, middle_id, egui::Sense::click_and_drag())
                    .on_hover_and_drag_cursor(CursorIcon::Move);

                let left_response = ui
                    .interact(left_edge_rect, left_edge_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(CursorIcon::ResizeWest);

                let right_response = ui
                    .interact(right_edge_rect, right_edge_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(CursorIcon::ResizeEast);

                // Use "smart_aim" to find a natural length of the time interval
                let aim_radius = ui.input().aim_radius();
                use egui::emath::smart_aim::best_in_range_f64;

                if left_response.dragged() {
                    if let (Some(time_low), Some(time_high)) = (
                        time_ranges_ui.time_from_x(pointer_pos.x - aim_radius),
                        time_ranges_ui.time_from_x(pointer_pos.x + aim_radius),
                    ) {
                        // TODO(emilk): snap to absolute time too
                        let low_length = selected_range.max - time_low;
                        let high_length = selected_range.max - time_high;
                        let best_length = TimeReal::from(best_in_range_f64(
                            low_length.as_f64(),
                            high_length.as_f64(),
                        ));

                        selected_range.min = selected_range.max - best_length;

                        if selected_range.min > selected_range.max {
                            std::mem::swap(&mut selected_range.min, &mut selected_range.max);
                            ui.memory().set_dragged_id(right_edge_id);
                        }

                        time_ctrl.set_loop_selection(selected_range);
                        time_ctrl.looping = Looping::Selection;
                    }
                }

                if right_response.dragged() {
                    if let (Some(time_low), Some(time_high)) = (
                        time_ranges_ui.time_from_x(pointer_pos.x - aim_radius),
                        time_ranges_ui.time_from_x(pointer_pos.x + aim_radius),
                    ) {
                        // TODO(emilk): snap to absolute time too
                        let low_length = time_low - selected_range.min;
                        let high_length = time_high - selected_range.min;
                        let best_length = TimeReal::from(best_in_range_f64(
                            low_length.as_f64(),
                            high_length.as_f64(),
                        ));

                        selected_range.max = selected_range.min + best_length;

                        if selected_range.min > selected_range.max {
                            std::mem::swap(&mut selected_range.min, &mut selected_range.max);
                            ui.memory().set_dragged_id(left_edge_id);
                        }

                        time_ctrl.set_loop_selection(selected_range);
                        time_ctrl.looping = Looping::Selection;
                    }
                }

                if middle_response.clicked() {
                    // Click to toggle looping
                    if time_ctrl.looping == Looping::Selection {
                        time_ctrl.looping = Looping::Off;
                    } else {
                        time_ctrl.looping = Looping::Selection;
                    }
                }

                if middle_response.dragged() {
                    (|| {
                        let min_x = time_ranges_ui.x_from_time(selected_range.min)?;
                        let max_x = time_ranges_ui.x_from_time(selected_range.max)?;

                        let min_x = min_x + ui.input().pointer.delta().x;
                        let max_x = max_x + ui.input().pointer.delta().x;

                        let min_time = time_ranges_ui.time_from_x(min_x)?;
                        let max_time = time_ranges_ui.time_from_x(max_x)?;

                        let mut new_range = TimeRangeF::new(min_time, max_time);

                        if egui::emath::almost_equal(
                            selected_range.length().as_f32(),
                            new_range.length().as_f32(),
                            1e-5,
                        ) {
                            // Avoid numerical inaccuracies: maintain length if very close
                            new_range.max = new_range.min + selected_range.length();
                        }

                        time_ctrl.set_loop_selection(new_range);
                        if ui.input().pointer.is_moving() {
                            time_ctrl.looping = Looping::Selection;
                        }
                        Some(())
                    })();
                }
            }
        }
    }

    // Start new selection?
    if let Some(pointer_pos) = pointer_pos {
        let is_anything_being_dragged = ui.memory().is_anything_being_dragged();
        if is_pointer_in_timeline
            && !is_anything_being_dragged
            && ui.input().pointer.primary_down()
            && ui.input().modifiers.shift_only()
        {
            if let Some(time) = time_ranges_ui.time_from_x(pointer_pos.x) {
                time_ctrl.set_loop_selection(TimeRangeF::point(time));
                time_ctrl.looping = Looping::Selection;
                ui.memory().set_dragged_id(right_edge_id);
            }
        }
    }
}

fn paint_range_text(
    time_ctrl: &mut TimeControl,
    selected_range: TimeRangeF,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    selection_rect: Rect,
) {
    if selected_range.min <= TimeInt::BEGINNING {
        return; // huge time selection, don't show a confusing times
    }

    let text_color = ui.visuals().strong_text_color();

    let arrow_color = text_color.gamma_multiply(0.75);
    let arrow_stroke = Stroke::new(1.0, arrow_color);

    fn paint_arrow_from_to(painter: &egui::Painter, origin: Pos2, to: Pos2, stroke: Stroke) {
        use egui::emath::Rot2;
        let vec = to - origin;
        let rot = Rot2::from_angle(std::f32::consts::TAU / 10.0);
        let tip_length = 6.0;
        let tip = origin + vec;
        let dir = vec.normalized();
        painter.line_segment([origin, tip], stroke);
        painter.line_segment([tip, tip - tip_length * (rot * dir)], stroke);
        painter.line_segment([tip, tip - tip_length * (rot.inverse() * dir)], stroke);
    }

    let range_text = format_duration(time_ctrl.time_type(), selected_range.length().abs());
    if range_text.is_empty() {
        return;
    }

    let font_id = egui::TextStyle::Small.resolve(ui.style());
    let text_rect = painter.text(
        selection_rect.center(),
        Align2::CENTER_CENTER,
        range_text,
        font_id,
        text_color,
    );

    // Draw arrows on either side, if we have the space for it:
    let text_rect = text_rect.expand(2.0); // Add some margin around text
    let selection_rect = selection_rect.shrink(1.0); // Add some margin inside of the selection rect
    let min_arrow_length = 12.0;
    if selection_rect.left() + min_arrow_length <= text_rect.left() {
        paint_arrow_from_to(
            painter,
            text_rect.left_center(),
            selection_rect.left_center(),
            arrow_stroke,
        );
    }
    if text_rect.right() + min_arrow_length <= selection_rect.right() {
        paint_arrow_from_to(
            painter,
            text_rect.right_center(),
            selection_rect.right_center(),
            arrow_stroke,
        );
    }
}

/// Human-readable description of a duration
pub fn format_duration(time_typ: TimeType, duration: TimeReal) -> String {
    match time_typ {
        TimeType::Time => Duration::from(duration).to_string(),
        TimeType::Sequence => duration.round().as_i64().to_string(), // TODO(emilk): show real part?
    }
}

/// A vertical line that shows the current time.
fn time_marker_ui(
    time_ranges_ui: &TimeRangesUi,
    time_ctrl: &mut TimeControl,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    timeline_rect: &Rect,
) {
    // timeline_rect: top part with the second ticks and time marker

    let pointer_pos = ui.input().pointer.hover_pos();

    // ------------------------------------------------

    let time_drag_id = ui.id().with("time_drag_id");

    let mut is_hovering = false;

    let timeline_cursor_icon = CursorIcon::ResizeHorizontal;

    let is_hovering_the_loop_selection = ui.output().cursor_icon != CursorIcon::Default; // A kind of hacky proxy

    let is_anything_being_dragged = ui.memory().is_anything_being_dragged();

    let interact_radius = ui.style().interaction.resize_grab_radius_side;

    // show current time as a line:
    if let Some(time) = time_ctrl.time() {
        if let Some(x) = time_ranges_ui.x_from_time(time) {
            let line_rect =
                Rect::from_x_y_ranges(x..=x, timeline_rect.top()..=ui.max_rect().bottom())
                    .expand(interact_radius);

            let response = ui
                .interact(line_rect, time_drag_id, egui::Sense::drag())
                .on_hover_and_drag_cursor(timeline_cursor_icon);

            is_hovering = !is_anything_being_dragged && response.hovered();

            if response.dragged() {
                if let Some(pointer_pos) = pointer_pos {
                    if let Some(time) = time_ranges_ui.time_from_x(pointer_pos.x) {
                        let time = time_ranges_ui.clamp_time(time);
                        time_ctrl.set_time(time);
                        time_ctrl.pause();
                    }
                }
            }

            let stroke = if response.dragged() {
                ui.style().visuals.widgets.active.bg_stroke
            } else if is_hovering {
                ui.style().visuals.widgets.hovered.bg_stroke
            } else {
                ui.visuals().widgets.inactive.fg_stroke
            };
            paint_time_cursor(
                time_area_painter,
                x,
                timeline_rect.top()..=ui.max_rect().bottom(),
                stroke,
            );
        }
    }

    // "click here to view time here"
    if let Some(pointer_pos) = pointer_pos {
        let is_pointer_in_timeline_rect = timeline_rect.contains(pointer_pos);

        // Show preview?
        if !is_hovering
            && is_pointer_in_timeline_rect
            && !is_anything_being_dragged
            && !is_hovering_the_loop_selection
        {
            time_area_painter.vline(
                pointer_pos.x,
                timeline_rect.top()..=ui.max_rect().bottom(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
            ui.output().cursor_icon = timeline_cursor_icon; // preview!
        }

        // Click to move time here:
        if ui.input().pointer.primary_down()
            && is_pointer_in_timeline_rect
            && !is_anything_being_dragged
            && !is_hovering_the_loop_selection
        {
            if let Some(time) = time_ranges_ui.time_from_x(pointer_pos.x) {
                let time = time_ranges_ui.clamp_time(time);
                time_ctrl.set_time(time);
                time_ctrl.pause();
                ui.memory().set_dragged_id(time_drag_id);
            }
        }
    }
}

pub fn paint_time_cursor(
    painter: &egui::Painter,
    x: f32,
    y: RangeInclusive<f32>,
    stroke: egui::Stroke,
) {
    let y_min = *y.start();
    let y_max = *y.end();

    let stroke = egui::Stroke {
        width: 1.5 * stroke.width,
        color: stroke.color,
    };

    let w = 10.0;
    let triangle = vec![
        pos2(x - 0.5 * w, y_min), // left top
        pos2(x + 0.5 * w, y_min), // right top
        pos2(x, y_min + w),       // bottom
    ];
    painter.add(egui::Shape::convex_polygon(
        triangle,
        stroke.color,
        egui::Stroke::NONE,
    ));
    painter.vline(x, (y_min + w)..=y_max, stroke);
}

// ----------------------------------------------------------------------------

/// The ideal gap between time segments.
///
/// This is later shrunk via [`GAP_EXPANSION_FRACTION`].
const MAX_GAP: f32 = 40.0;

/// How much of the gap use up to expand segments visually to either side?
const GAP_EXPANSION_FRACTION: f32 = 1.0 / 4.0;

/// Sze of the gap between time segments.
fn gap_width(x_range: &RangeInclusive<f32>, segments: &[TimeRange]) -> f32 {
    let num_gaps = segments.len().saturating_sub(1);
    if num_gaps == 0 {
        // gap width doesn't matter when there are no gaps
        MAX_GAP
    } else {
        // shrink gaps if there are a lot of them
        let width = *x_range.end() - *x_range.start();
        (width / (num_gaps as f32)).at_most(MAX_GAP)
    }
}

/// Find a nice view of everything.
fn view_everything(x_range: &RangeInclusive<f32>, timeline_axis: &TimelineAxis) -> TimeView {
    let gap_width = gap_width(x_range, &timeline_axis.ranges);
    let num_gaps = timeline_axis.ranges.len().saturating_sub(1);
    let width = *x_range.end() - *x_range.start();
    let width_sans_gaps = width - num_gaps as f32 * gap_width;

    let factor = if width_sans_gaps > 0.0 {
        width / width_sans_gaps
    } else {
        1.0 // too narrow to fit everything anyways
    };

    let min = timeline_axis.min();
    let time_spanned = timeline_axis.sum_time_lengths().as_f64() * factor as f64;

    TimeView {
        min: min.into(),
        time_spanned,
    }
}

struct Segment {
    /// Matches [`Self::time`] (linear transform).
    x: RangeInclusive<f32>,

    /// Matches [`Self::x`] (linear transform).
    time: TimeRangeF,

    /// does NOT match any of the above. Instead this is a tight bound.
    tight_time: TimeRange,
}

/// Represents a compressed view of time.
/// It does so by breaking up the timeline in linear segments.
///
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
                min: TimeReal::from(0),
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
                let range_width = range.length().as_f32() * points_per_time;
                let right = left + range_width;
                let x_range = left..=right;
                left = right + gap_width;

                let tight_time = *range;

                // expand each span outwards a bit to make selection of outer data points easier.
                // Also gives zero-width segments some width!
                let expansion = GAP_EXPANSION_FRACTION * gap_width;
                let x_range = (*x_range.start() - expansion)..=(*x_range.end() + expansion);

                let range = if range.min == range.max {
                    TimeRangeF::from(*range) // don't expand zero-width segments (e.g. `TimeInt::BEGINNING`).
                } else {
                    let time_expansion = TimeReal::from(expansion / points_per_time);
                    TimeRangeF::new(range.min - time_expansion, range.max + time_expansion)
                };

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

    /// Clamp the time to the valid ranges.
    ///
    /// Used when user is dragging the time handle.
    fn clamp_time(&self, mut time: TimeReal) -> TimeReal {
        if let (Some(first), Some(last)) = (self.segments.first(), self.segments.last()) {
            time = time.clamp(
                TimeReal::from(first.tight_time.min),
                TimeReal::from(last.tight_time.max),
            );

            // Special: don't allow users dragging time between
            // BEGINNING (-∞ = timeless data) and some real time.
            // Otherwise we get weird times (e.g. dates in 1923).
            // Selecting times between other segments is not as problematic, as all other segments are
            // real times, so interpolating between them always produces valid times.
            // By disallowing times between BEGINNING and the first real segment,
            // we also disallow users dragging the time to be between -∞ and the
            // real beginning of their data. That further highlights the specialness of -∞.
            // Furthermore, we want users to have a smooth experience dragging the time handle anywhere else.
            if first.tight_time == TimeRange::point(TimeInt::BEGINNING) {
                if let Some(second) = self.segments.get(1) {
                    if TimeInt::BEGINNING < time && time < second.tight_time.min {
                        time = TimeReal::from(second.tight_time.min);
                    }
                }
            }
        }
        time
    }

    /// Make sure the time is not between segments.
    ///
    /// This is so that the playback doesn't get stuck between segments.
    fn snap_time_to_segments(&self, value: TimeReal) -> TimeReal {
        for segment in &self.segments {
            if value < segment.time.min {
                return segment.time.min;
            } else if value <= segment.time.max {
                return value;
            }
        }
        value
    }

    // Make sure playback time doesn't get stuck between non-continuos regions:
    fn snap_time_control(&self, ctx: &mut ViewerContext<'_>) {
        if !ctx.rec_cfg.time_ctrl.is_playing() {
            return;
        }

        // Make sure time doesn't get stuck between non-continuos regions:
        if let Some(time) = ctx.rec_cfg.time_ctrl.time() {
            let time = self.snap_time_to_segments(time);
            ctx.rec_cfg.time_ctrl.set_time(time);
        } else if let Some(selection) = ctx.rec_cfg.time_ctrl.loop_selection() {
            let snapped_min = self.snap_time_to_segments(selection.min);
            let snapped_max = self.snap_time_to_segments(selection.max);

            let min_was_good = selection.min == snapped_min;
            let max_was_good = selection.max == snapped_max;

            if min_was_good || max_was_good {
                return;
            }

            // Keeping max works better when looping
            ctx.rec_cfg.time_ctrl.set_loop_selection(TimeRangeF::new(
                snapped_max - selection.length(),
                snapped_max,
            ));
        }
    }

    fn x_from_time(&self, needle_time: TimeReal) -> Option<f32> {
        let first_segment = self.segments.first()?;
        let mut last_x = *first_segment.x.start();
        let mut last_time = first_segment.time.min;

        if needle_time < last_time {
            // extrapolate:
            return Some(last_x - self.points_per_time * (last_time - needle_time).as_f32());
        }

        for segment in &self.segments {
            if needle_time < segment.time.min {
                let t = TimeRangeF::new(last_time, segment.time.min).inverse_lerp(needle_time);
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
        Some(last_x + self.points_per_time * (needle_time - last_time).as_f32())
    }

    fn time_from_x(&self, needle_x: f32) -> Option<TimeReal> {
        let first_segment = self.segments.first()?;
        let mut last_x = *first_segment.x.start();
        let mut last_time = first_segment.time.min;

        if needle_x < last_x {
            // extrapolate:
            return Some(last_time + TimeReal::from((needle_x - last_x) / self.points_per_time));
        }

        for segment in &self.segments {
            if needle_x < *segment.x.start() {
                let t = remap(needle_x, last_x..=*segment.x.start(), 0.0..=1.0);
                return Some(TimeRangeF::new(last_time, segment.time.min).lerp(t));
            } else if needle_x <= *segment.x.end() {
                let t = remap(needle_x, segment.x.clone(), 0.0..=1.0);
                return Some(segment.time.lerp(t));
            } else {
                last_x = *segment.x.end();
                last_time = segment.time.max;
            }
        }

        // extrapolate:
        Some(last_time + TimeReal::from((needle_x - last_x) / self.points_per_time))
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
            let dt = segment.time.length().as_f32();
            if dx > 0.0 && dt > 0.0 {
                return Some(dx / dt);
            }
        }
        None
    }
}

fn paint_time_range_ticks(
    ui: &mut egui::Ui,
    rect: &Rect,
    time_type: TimeType,
    time_range: &TimeRangeF,
) -> Vec<Shape> {
    let font_id = egui::TextStyle::Small.resolve(ui.style());

    match time_type {
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
                } else if spacing_ns == 60 * 60 * 1_000_000_000 {
                    spacing_ns * 12 // to 12 h
                } else if spacing_ns == 12 * 60 * 60 * 1_000_000_000 {
                    spacing_ns * 2 // to a day
                } else {
                    spacing_ns.checked_mul(10).unwrap_or(spacing_ns) // multiple of ten days
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
                    // We are in the sub-second resolution.
                    // Showing the full time (HH:MM:SS.XXX or 3h 2m 6s …) becomes too long,
                    // so instead we switch to showing the time as milliseconds since the last whole second:
                    let ms = relative_ns as f64 * 1e-6;
                    if relative_ns % 1_000_000 == 0 {
                        format!("{:+.0} ms", ms)
                    } else if relative_ns % 100_000 == 0 {
                        format!("{:+.1} ms", ms)
                    } else if relative_ns % 10_000 == 0 {
                        format!("{:+.2} ms", ms)
                    } else if relative_ns % 1_000 == 0 {
                        format!("{:+.3} ms", ms)
                    } else if relative_ns % 100 == 0 {
                        format!("{:+.4} ms", ms)
                    } else if relative_ns % 10 == 0 {
                        format!("{:+.5} ms", ms)
                    } else {
                        format!("{:+.6} ms", ms)
                    }
                }
            }

            paint_ticks(
                &ui.fonts(),
                ui.visuals().dark_mode,
                &font_id,
                rect,
                &ui.clip_rect(),
                time_range, // ns
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
                time_range,
                next_power_of_10,
                |seq| format!("#{seq}"),
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_ticks(
    fonts: &egui::epaint::Fonts,
    dark_mode: bool,
    font_id: &egui::FontId,
    canvas: &Rect,
    clip_rect: &Rect,
    time_range: &TimeRangeF,
    next_time_step: fn(i64) -> i64,
    format_tick: fn(i64) -> String,
) -> Vec<egui::Shape> {
    crate::profile_function!();

    let color_from_alpha = |alpha: f32| -> Color32 {
        if dark_mode {
            Rgba::from_white_alpha(alpha * alpha).into()
        } else {
            Rgba::from_black_alpha(alpha).into()
        }
    };

    let x_from_time = |time: i64| -> f32 {
        let t = (TimeReal::from(time) - time_range.min).as_f32()
            / (time_range.max - time_range.min).as_f32();
        lerp(canvas.x_range(), t)
    };

    let visible_rect = clip_rect.intersect(*canvas);
    let mut shapes = vec![];

    if !visible_rect.is_positive() {
        return shapes;
    }

    let width_time = (time_range.max - time_range.min).as_f32();
    let points_per_time = canvas.width() / width_time;
    let minimum_small_line_spacing = 4.0;
    let expected_text_width = 60.0;

    let line_strength_from_spacing = |spacing_time: i64| -> f32 {
        let next_tick_magnitude = next_time_step(spacing_time) / spacing_time;
        remap_clamp(
            spacing_time as f32 * points_per_time,
            minimum_small_line_spacing..=(next_tick_magnitude as f32 * minimum_small_line_spacing),
            0.0..=1.0,
        )
    };

    let text_color_from_spacing = |spacing_time: i64| -> Color32 {
        let alpha = remap_clamp(
            spacing_time as f32 * points_per_time,
            expected_text_width..=(3.0 * expected_text_width),
            0.0..=0.5,
        );
        color_from_alpha(alpha)
    };

    let max_small_lines = canvas.width() / minimum_small_line_spacing;
    let mut small_spacing_time = 1;
    while width_time / (small_spacing_time as f32) > max_small_lines {
        small_spacing_time = next_time_step(small_spacing_time);
    }
    let medium_spacing_time = next_time_step(small_spacing_time);
    let big_spacing_time = next_time_step(medium_spacing_time);

    // We fade in lines as we zoom in:
    let big_line_strength = line_strength_from_spacing(big_spacing_time);
    let medium_line_strength = line_strength_from_spacing(medium_spacing_time);
    let small_line_strength = line_strength_from_spacing(small_spacing_time);

    let big_line_color = color_from_alpha(0.4 * big_line_strength);
    let medium_line_color = color_from_alpha(0.4 * medium_line_strength);
    let small_line_color = color_from_alpha(0.4 * small_line_strength);

    let big_text_color = text_color_from_spacing(big_spacing_time);
    let medium_text_color = text_color_from_spacing(medium_spacing_time);
    let small_text_color = text_color_from_spacing(small_spacing_time);

    let mut current_time =
        time_range.min.floor().as_i64() / small_spacing_time * small_spacing_time;

    while current_time <= time_range.max.ceil().as_i64() {
        let line_x = x_from_time(current_time);

        if visible_rect.min.x <= line_x && line_x <= visible_rect.max.x {
            let medium_line = current_time % medium_spacing_time == 0;
            let big_line = current_time % big_spacing_time == 0;

            let (height_factor, line_color, text_color) = if big_line {
                (medium_line_strength, big_line_color, big_text_color)
            } else if medium_line {
                (small_line_strength, medium_line_color, medium_text_color)
            } else {
                (0.0, small_line_color, small_text_color)
            };

            // Make line higher if it is stronger:
            let line_top = lerp(canvas.y_range(), lerp(0.75..=0.5, height_factor));

            shapes.push(egui::Shape::line_segment(
                [pos2(line_x, line_top), pos2(line_x, canvas.max.y)],
                Stroke::new(1.0, line_color),
            ));

            if text_color != Color32::TRANSPARENT {
                let text = format_tick(current_time);
                let text_x = line_x + 4.0;

                shapes.push(egui::Shape::text(
                    fonts,
                    pos2(text_x, lerp(canvas.y_range(), 0.5)),
                    Align2::LEFT_CENTER,
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

#[test]
fn test_time_ranges_ui() {
    let time_range_ui = TimeRangesUi::new(
        100.0..=1000.0,
        TimeView {
            min: TimeReal::from(0.5),
            time_spanned: 14.2,
        },
        &[
            TimeRange::new(TimeInt::from(0), TimeInt::from(0)),
            TimeRange::new(TimeInt::from(1), TimeInt::from(5)),
            TimeRange::new(TimeInt::from(10), TimeInt::from(100)),
        ],
    );

    // Sanity check round-tripping:
    for segment in &time_range_ui.segments {
        let pixel_precision = 0.5;

        assert_eq!(
            time_range_ui.time_from_x(*segment.x.start()).unwrap(),
            segment.time.min
        );
        assert_eq!(
            time_range_ui.time_from_x(*segment.x.end()).unwrap(),
            segment.time.max
        );

        if segment.time.is_empty() {
            let x = time_range_ui.x_from_time(segment.time.min).unwrap();
            let mid_x = lerp(segment.x.clone(), 0.5);
            assert!((mid_x - x).abs() < pixel_precision);
        } else {
            let min_x = time_range_ui.x_from_time(segment.time.min).unwrap();
            assert!((min_x - *segment.x.start()).abs() < pixel_precision);

            let max_x = time_range_ui.x_from_time(segment.time.max).unwrap();
            assert!((max_x - *segment.x.end()).abs() < pixel_precision);
        }
    }
}
