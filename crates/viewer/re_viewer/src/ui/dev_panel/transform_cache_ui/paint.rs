use ahash::HashSet;
use egui::{FontSelection, TextWrapMode, WidgetText};
use re_log_types::TimeInt;
use re_sdk_types::TransformFrameIdHash;
use re_ui::UiExt as _;
use re_ui::list_item;
use re_viewer_context::external::re_tf::transform_cache_snapshot::EdgeSource;

use super::LayoutDirection;
use super::layout::Layout;
use super::model::{Edge, Model, Node, SubspaceKind, is_implicit_frame};

const EDGE_STROKE_WIDTH: f32 = 1.5;
const EDGE_HIGHLIGHT_STROKE_WIDTH: f32 = EDGE_STROKE_WIDTH * 2.;
const EDGE_HIT_RADIUS: f32 = EDGE_STROKE_WIDTH * 4.;
const FRAME_PROPERTY_MIN_WIDTH: f32 = 300.0;
const LEGEND_FILL_OPACITY: f32 = 0.5;
const SCENE_ICON_SCALE: f32 = 1.25;

/// Thing currently hovered in the transform-cache scene.
#[derive(Debug, Clone, Copy)]
enum HoveredTransformItem {
    Node(TransformFrameIdHash),
    Edge(usize),
    SharedFork(TransformFrameIdHash),
}

/// Frames and transforms highlighted for the current hover state.
#[derive(Default)]
struct HighlightedTransformPath {
    nodes: HashSet<TransformFrameIdHash>,
    edges: HashSet<usize>,
}

/// Orientation-independent coordinates for a shared fork path.
struct ForkGeometry {
    segments: [[egui::Pos2; 2]; 2],
    joints: Vec<egui::Pos2>,
}

/// Orientation-independent coordinates for the shared part of one edge's fork path.
struct ForkEdgePath {
    segments: [[egui::Pos2; 2]; 2],
    joints: [egui::Pos2; 2],
}

impl HighlightedTransformPath {
    fn new(hovered_item: Option<HoveredTransformItem>, model: &Model) -> Self {
        let mut highlighted_path = Self::default();

        match hovered_item {
            Some(HoveredTransformItem::Node(frame)) => {
                highlighted_path.nodes.insert(frame);
                let (ancestors, edge_indices) = model.path_to_roots(frame);
                highlighted_path.nodes.extend(ancestors);
                highlighted_path.edges = edge_indices;
            }
            Some(HoveredTransformItem::Edge(edge_index)) => {
                if let Some(edge) = model.snapshot.edges.get(edge_index) {
                    highlighted_path.nodes.insert(edge.parent);
                    highlighted_path.nodes.insert(edge.child);
                    highlighted_path.edges.insert(edge_index);
                }
            }
            Some(HoveredTransformItem::SharedFork(parent)) => {
                highlighted_path.nodes.insert(parent);
                highlighted_path.edges.extend(
                    model
                        .edge_indices_by_parent
                        .get(&parent)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            None => {}
        }

        highlighted_path
    }
}

impl ForkGeometry {
    fn new(
        parent: TransformFrameIdHash,
        model: &Model,
        layout: &Layout<'_>,
        node_size: egui::Vec2,
    ) -> Option<Self> {
        let child_edge_indices = model.edge_indices_by_parent.get(&parent)?;
        if child_edge_indices.len() <= 1 {
            return None;
        }

        let parent_pos = layout.positions.get(&parent).copied()?;
        let child_positions = child_edge_indices
            .iter()
            .filter_map(|&edge_index| {
                layout
                    .positions
                    .get(&model.snapshot.edges[edge_index].child)
                    .copied()
            })
            .collect::<Vec<_>>();
        let first_child_pos = child_positions.first().copied()?;

        let fork_depth = layout.fork_depth_coordinate(parent_pos, first_child_pos, node_size);
        let parent_exit = node_exit(parent_pos, layout.direction, node_size);
        let parent_fork = pos_from_depth_cross(
            fork_depth,
            cross_coordinate(parent_exit, layout.direction),
            layout.direction,
        );
        let child_forks = child_positions
            .iter()
            .map(|&child_pos| {
                let child_entry = node_entry(child_pos, layout.direction, node_size);
                pos_from_depth_cross(
                    fork_depth,
                    cross_coordinate(child_entry, layout.direction),
                    layout.direction,
                )
            })
            .collect::<Vec<_>>();
        let first_child_fork = child_forks.first().copied()?;
        let (min_child_cross, max_child_cross) = child_forks.iter().skip(1).fold(
            (
                cross_coordinate(first_child_fork, layout.direction),
                cross_coordinate(first_child_fork, layout.direction),
            ),
            |(min_cross, max_cross), child_fork| {
                let cross = cross_coordinate(*child_fork, layout.direction);
                (min_cross.min(cross), max_cross.max(cross))
            },
        );
        let bus_start = pos_from_depth_cross(fork_depth, min_child_cross, layout.direction);
        let bus_end = pos_from_depth_cross(fork_depth, max_child_cross, layout.direction);

        let mut joints = Vec::with_capacity(child_forks.len() + 1);
        joints.push(parent_fork);
        joints.extend(child_forks);

        Some(Self {
            segments: [[parent_exit, parent_fork], [bus_start, bus_end]],
            joints,
        })
    }
}

impl ForkEdgePath {
    fn new(edge: &Edge, layout: &Layout<'_>, node_size: egui::Vec2) -> Option<Self> {
        let parent_pos = layout.positions.get(&edge.parent).copied()?;
        let child_pos = layout.positions.get(&edge.child).copied()?;
        let fork_depth = layout.fork_depth_coordinate(parent_pos, child_pos, node_size);
        let parent_exit = node_exit(parent_pos, layout.direction, node_size);
        let parent_fork = pos_from_depth_cross(
            fork_depth,
            cross_coordinate(parent_exit, layout.direction),
            layout.direction,
        );
        let child_entry = node_entry(child_pos, layout.direction, node_size);
        let child_fork = pos_from_depth_cross(
            fork_depth,
            cross_coordinate(child_entry, layout.direction),
            layout.direction,
        );

        Some(Self {
            segments: [[parent_exit, parent_fork], [parent_fork, child_fork]],
            joints: [parent_fork, child_fork],
        })
    }
}

/// Draws the legend as a fixed overlay over the scene widget.
pub(super) fn scene_legend_ui(ui: &egui::Ui, scene_ui_rect: egui::Rect) {
    let tokens = ui.tokens();
    // Hide legend overlay if we don't have enough vertical space.
    let min_scene_height_for_legend = ui.spacing().interact_size.y * 2.0
        + ui.spacing().item_spacing.y
        + tokens.text_to_icon_padding() * 2.0;
    if scene_ui_rect.height() < min_scene_height_for_legend {
        return;
    }

    let legend_inset = f32::from(tokens.view_padding());
    let anchor_offset = egui::vec2(
        scene_ui_rect.right() - ui.ctx().content_rect().right() - legend_inset,
        scene_ui_rect.top() - ui.ctx().content_rect().top() + legend_inset,
    );

    egui::Area::new(ui.id().with("transform_cache_legend"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, anchor_offset)
        .show(ui.ctx(), |ui| {
            let tokens = ui.tokens();
            egui::Frame::new()
                .fill(tokens.panel_bg_color.gamma_multiply(LEGEND_FILL_OPACITY))
                .stroke(egui::Stroke::new(
                    1.0,
                    tokens.widget_noninteractive_bg_stroke,
                ))
                .corner_radius(tokens.small_corner_radius())
                .inner_margin(tokens.text_to_icon_padding())
                .show(ui, |ui| {
                    // The legend is anchored to the scene UI, not the zoomable content, so it
                    // remains readable while panning and zooming the model.
                    legend_ui(ui);
                });
        });
}

/// Draws transform-cache geometry, hover regions, and hover tooltips inside the scene.
pub(super) fn draw_transform_cache_contents(
    ui: &mut egui::Ui,
    model: &Model,
    layout: &Layout<'_>,
    node_size: egui::Vec2,
    content_rect: egui::Rect,
) {
    let mut hovered_item = None;
    let scene_response = ui.allocate_rect(content_rect, egui::Sense::hover());

    // Allocate node hover regions before drawing so nodes take precedence over nearby edges.
    for node in &model.snapshot.frames {
        let Some(pos) = layout.positions.get(&node.id) else {
            continue;
        };
        let response = ui.allocate_rect(
            egui::Rect::from_min_size(*pos, node_size),
            egui::Sense::hover(),
        );
        if response.hovered() {
            hovered_item = Some(HoveredTransformItem::Node(node.id));
        }
        response.on_hover_ui(|ui| node_tooltip_ui(ui, model, node));
    }

    if hovered_item.is_none()
        && let Some(hovered_edge_index) = scene_response.hover_pos().and_then(|pos| {
            nearest_edge(
                pos,
                model,
                layout,
                node_size,
                scene_icon_size(ui),
                EDGE_HIT_RADIUS,
            )
        })
    {
        hovered_item = Some(HoveredTransformItem::Edge(hovered_edge_index));
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            ui.id().with("transform_edge_tooltip"),
            egui::PopupAnchor::Pointer,
        )
        .at_pointer()
        .show(|ui| edge_tooltip_ui(ui, model, &model.snapshot.edges[hovered_edge_index]));
    }

    if hovered_item.is_none()
        && let Some(hovered_parent) = scene_response
            .hover_pos()
            .and_then(|pos| nearest_shared_fork(pos, model, layout, node_size, EDGE_HIT_RADIUS))
    {
        hovered_item = Some(HoveredTransformItem::SharedFork(hovered_parent));
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            ui.id().with("transform_shared_fork_tooltip"),
            egui::PopupAnchor::Pointer,
        )
        .at_pointer()
        .show(|ui| shared_fork_tooltip_ui(ui, model, hovered_parent));
    }
    let highlighted_path = HighlightedTransformPath::new(hovered_item, model);

    let painter = ui.painter();
    painter.rect_filled(content_rect, 0.0, ui.tokens().faint_bg_color);

    // Sibling edges share a neutral fork path so only the terminal segment carries per-transform
    // interaction and time-kind styling.
    draw_shared_fork_segments(painter, model, layout, node_size, ui);

    // Draw highlighted ancestry through shared fork segments, not just through the terminal edge.
    for (edge_index, edge) in model.snapshot.edges.iter().enumerate() {
        let highlighted = highlighted_path.edges.contains(&edge_index);
        if highlighted && model.edge_starts_at_shared_fork(edge) {
            draw_shared_fork_path(
                painter,
                edge,
                layout,
                node_size,
                egui::Stroke::new(EDGE_HIGHLIGHT_STROKE_WIDTH, edge_color(ui)),
            );
        }
    }

    // Draw transform-specific edge terminals above the shared fork segments, but below nodes.
    for (edge_index, edge) in model.snapshot.edges.iter().enumerate() {
        let Some((start, end)) = edge_unique_segment(edge, model, layout, node_size) else {
            continue;
        };
        let color = edge_color(ui);
        let highlighted = highlighted_path.edges.contains(&edge_index);
        draw_edge_line(
            painter,
            start,
            end,
            egui::Stroke::new(
                if highlighted {
                    EDGE_HIGHLIGHT_STROKE_WIDTH
                } else {
                    EDGE_STROKE_WIDTH
                },
                color,
            ),
        );
        edge_time_icon(edge.time)
            .as_image()
            .tint(color)
            .paint_at(ui, edge_time_icon_rect(start, end, scene_icon_size(ui)));
    }

    for node in &model.snapshot.frames {
        let Some(pos) = layout.positions.get(&node.id) else {
            continue;
        };
        let tokens = ui.tokens();
        let highlighted = highlighted_path.nodes.contains(&node.id);
        let rect = egui::Rect::from_min_size(*pos, node_size);
        painter.rect(
            rect,
            tokens.small_corner_radius(),
            if highlighted {
                tokens.widget_hovered_bg_fill
            } else {
                tokens.panel_bg_color
            },
            if highlighted {
                tokens.focus_outline_stroke
            } else {
                egui::Stroke::new(2.0, tokens.widget_noninteractive_bg_stroke)
            },
            egui::StrokeKind::Inside,
        );

        let icon_size = scene_icon_size(ui);
        let icon_inset = tokens.text_to_icon_padding();
        let mut text_rect = rect.shrink2(egui::vec2(2.5 * icon_inset, 1.25 * icon_inset));
        if !node.has_transform {
            text_rect.min.x += icon_size.x + icon_inset;
        }
        text_rect.max.x -= icon_size.x + icon_inset;
        let galley = WidgetText::from(node.label.as_str()).into_galley(
            ui,
            Some(TextWrapMode::Wrap),
            text_rect.width(),
            FontSelection::Style(egui::TextStyle::Body),
        );
        let text_pos = egui::pos2(
            text_rect.center().x - galley.size().x.min(text_rect.width()) / 2.0,
            text_rect.center().y - galley.size().y.min(text_rect.height()) / 2.0,
        );
        painter
            .with_clip_rect(text_rect)
            .galley(text_pos, galley, tokens.text_default);

        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(
                rect.right() - icon_inset - icon_size.x,
                rect.top() + icon_inset,
            ),
            icon_size,
        );
        subspace_icon(node.subspace_kind)
            .as_image()
            .tint(tokens.text_subdued)
            .paint_at(ui, icon_rect);

        if !node.has_transform {
            let warning_icon_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + icon_inset, rect.top() + icon_inset),
                icon_size,
            );
            re_ui::icons::WARNING
                .as_image()
                .tint(tokens.alert_warning.icon)
                .paint_at(ui, warning_icon_rect);
        }
    }
}

/// Draws the static/temporal edge-style legend contents.
fn legend_ui(ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        legend_item_ui(ui, "static", edge_color(ui), TimeInt::STATIC);
        legend_item_ui(ui, "temporal", edge_color(ui), TimeInt::new_temporal(0));
    });
}

/// Draws one edge time-kind icon and label in the legend.
fn legend_item_ui(ui: &mut egui::Ui, label: &str, color: egui::Color32, time: TimeInt) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(ui.tokens().small_icon_size, egui::Sense::hover());
        edge_time_icon(time)
            .as_image()
            .tint(color)
            .paint_at(ui, rect);
        ui.label(label);
    });
}

/// Returns the neutral color used for transform edges.
pub(super) fn edge_color(ui: &egui::Ui) -> egui::Color32 {
    ui.tokens().text_subdued
}

/// Returns the icon size used for zoomable scene contents.
fn scene_icon_size(ui: &egui::Ui) -> egui::Vec2 {
    ui.tokens().small_icon_size * SCENE_ICON_SCALE
}

/// Draws one transform edge segment.
pub(super) fn draw_edge_line(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    stroke: egui::Stroke,
) {
    painter.line_segment([start, end], stroke);
}

/// Returns the transform-specific terminal segment for an edge.
fn edge_unique_segment(
    edge: &Edge,
    model: &Model,
    layout: &Layout<'_>,
    node_size: egui::Vec2,
) -> Option<(egui::Pos2, egui::Pos2)> {
    let child_pos = layout.positions.get(&edge.child).copied()?;
    let parent_pos = layout.positions.get(&edge.parent).copied()?;
    let child_entry = node_entry(child_pos, layout.direction, node_size);

    let start = if model.edge_starts_at_shared_fork(edge) {
        let fork_depth = layout.fork_depth_coordinate(parent_pos, child_pos, node_size);
        pos_from_depth_cross(
            fork_depth,
            cross_coordinate(child_entry, layout.direction),
            layout.direction,
        )
    } else {
        let parent_exit = node_exit(parent_pos, layout.direction, node_size);
        pos_from_depth_cross(
            depth_coordinate(parent_exit, layout.direction),
            cross_coordinate(child_entry, layout.direction),
            layout.direction,
        )
    };

    Some((start, child_entry))
}

/// Returns the point where outgoing transform edges leave a node.
fn node_exit(
    node_pos: egui::Pos2,
    direction: LayoutDirection,
    node_size: egui::Vec2,
) -> egui::Pos2 {
    match direction {
        LayoutDirection::Horizontal => {
            egui::pos2(node_pos.x + node_size.x, node_pos.y + node_size.y / 2.0)
        }
        LayoutDirection::Vertical => {
            egui::pos2(node_pos.x + node_size.x / 2.0, node_pos.y + node_size.y)
        }
    }
}

/// Returns the point where incoming transform edges enter a node.
fn node_entry(
    node_pos: egui::Pos2,
    direction: LayoutDirection,
    node_size: egui::Vec2,
) -> egui::Pos2 {
    match direction {
        LayoutDirection::Horizontal => egui::pos2(node_pos.x, node_pos.y + node_size.y / 2.0),
        LayoutDirection::Vertical => egui::pos2(node_pos.x + node_size.x / 2.0, node_pos.y),
    }
}

/// Converts depth and cross-axis coordinates into scene-space coordinates.
fn pos_from_depth_cross(depth: f32, cross: f32, direction: LayoutDirection) -> egui::Pos2 {
    match direction {
        LayoutDirection::Horizontal => egui::pos2(depth, cross),
        LayoutDirection::Vertical => egui::pos2(cross, depth),
    }
}

/// Extracts the depth-axis coordinate from a scene-space position.
fn depth_coordinate(pos: egui::Pos2, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::Horizontal => pos.x,
        LayoutDirection::Vertical => pos.y,
    }
}

/// Extracts the cross-axis coordinate from a scene-space position.
fn cross_coordinate(pos: egui::Pos2, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::Horizontal => pos.y,
        LayoutDirection::Vertical => pos.x,
    }
}

/// Returns the icon used to distinguish static and temporal transforms.
fn edge_time_icon(time: TimeInt) -> &'static re_ui::Icon {
    if time.is_static() {
        &re_ui::icons::COMPONENT_STATIC
    } else {
        &re_ui::icons::COMPONENT_TEMPORAL
    }
}

/// Places the time-kind icon beside a terminal edge segment.
fn edge_time_icon_rect(start: egui::Pos2, end: egui::Pos2, icon_size: egui::Vec2) -> egui::Rect {
    let segment = end - start;
    let center = start.lerp(end, 0.5);
    let icon_offset = re_ui::DesignTokens::menu_button_padding();
    let offset = if segment.length_sq() <= f32::EPSILON {
        egui::vec2(0.0, -(icon_size.y / 2.0 + icon_offset))
    } else {
        let direction = segment.normalized();
        egui::vec2(direction.y, -direction.x) * (icon_size.y / 2.0 + icon_offset)
    };

    egui::Rect::from_center_size(center + offset, icon_size)
}

/// Draws the shared fork paths used by parents with multiple children.
fn draw_shared_fork_segments(
    painter: &egui::Painter,
    model: &Model,
    layout: &Layout<'_>,
    node_size: egui::Vec2,
    ui: &egui::Ui,
) {
    let color = edge_color(ui);
    let stroke = egui::Stroke::new(EDGE_STROKE_WIDTH, color);
    // These tiny disks hide rasterization gaps where perpendicular line caps meet.
    let intersection_radius = stroke.width / 2.0;

    for parent in shared_fork_parents(model) {
        let Some(geometry) = ForkGeometry::new(parent, model, layout, node_size) else {
            continue;
        };
        for [start, end] in geometry.segments {
            painter.line_segment([start, end], stroke);
        }
        for joint in geometry.joints {
            painter.circle_filled(joint, intersection_radius, color);
        }
    }
}

/// Draws the shared part of a highlighted path back to the root.
fn draw_shared_fork_path(
    painter: &egui::Painter,
    edge: &Edge,
    layout: &Layout<'_>,
    node_size: egui::Vec2,
    stroke: egui::Stroke,
) {
    let Some(path) = ForkEdgePath::new(edge, layout, node_size) else {
        return;
    };
    for [start, end] in path.segments {
        draw_edge_line(painter, start, end, stroke);
    }
    for joint in path.joints {
        painter.circle_filled(joint, stroke.width / 2.0, stroke.color);
    }
}

/// Computes the shortest distance from a point to a line segment.
fn distance_to_segment(point: egui::Pos2, start: egui::Pos2, end: egui::Pos2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }

    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + t * segment)
}

/// Finds the closest terminal transform edge under the pointer.
fn nearest_edge(
    point: egui::Pos2,
    model: &Model,
    layout: &Layout<'_>,
    node_size: egui::Vec2,
    icon_size: egui::Vec2,
    max_distance: f32,
) -> Option<usize> {
    // Hit-test only the transform-specific terminal segment; shared fork segments are decorative.
    model
        .snapshot
        .edges
        .iter()
        .enumerate()
        .filter_map(|(edge_index, edge)| {
            let (start, end) = edge_unique_segment(edge, model, layout, node_size)?;
            let icon_rect = edge_time_icon_rect(start, end, icon_size);
            let distance = if icon_rect.expand(max_distance).contains(point) {
                0.0
            } else {
                distance_to_segment(point, start, end)
            };
            Some((edge_index, distance))
        })
        .filter(|(_, distance)| *distance <= max_distance)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(edge_index, _)| edge_index)
}

/// Finds the closest shared fork path under the pointer.
fn nearest_shared_fork(
    point: egui::Pos2,
    model: &Model,
    layout: &Layout<'_>,
    node_size: egui::Vec2,
    max_distance: f32,
) -> Option<TransformFrameIdHash> {
    shared_fork_parents(model)
        .into_iter()
        .filter_map(|parent| {
            let min_distance = ForkGeometry::new(parent, model, layout, node_size)?
                .segments
                .into_iter()
                .map(|[start, end]| distance_to_segment(point, start, end))
                .min_by(f32::total_cmp)?;
            (min_distance <= max_distance).then_some((parent, min_distance))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(parent, _)| parent)
}

/// Returns parents with visible shared forks in deterministic edge order.
fn shared_fork_parents(model: &Model) -> Vec<TransformFrameIdHash> {
    let mut seen = HashSet::default();
    model
        .snapshot
        .edges
        .iter()
        .filter_map(|edge| {
            (model.edge_starts_at_shared_fork(edge) && seen.insert(edge.parent))
                .then_some(edge.parent)
        })
        .collect()
}

/// Draws the node hover tooltip.
fn node_tooltip_ui(ui: &mut egui::Ui, model: &Model, node: &Node) {
    list_item::list_item_scope(ui, "transform_node_hover", |ui| {
        ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Subspace").value_fn(
            |ui, _| {
                ui.horizontal(|ui| {
                    ui.small_icon(
                        subspace_icon(node.subspace_kind),
                        Some(ui.tokens().text_subdued),
                    );
                    ui.label(subspace_kind_label(node.subspace_kind));
                });
            },
        ));
        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Frame")
                .min_desired_width(FRAME_PROPERTY_MIN_WIDTH)
                .value_text(node.label.as_str()),
        );
        ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Kind").value_text(
            if is_implicit_frame(node) {
                "implicit"
            } else {
                "named"
            },
        ));
        if !node.has_transform {
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Warning").value_text("No transforms"),
            );
        }

        let ancestors = model.path_to_roots(node.id).0.len();
        let children = model.num_children(node.id);
        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Ancestors").value_text(ancestors.to_string()),
        );
        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Children").value_text(children.to_string()),
        );
    });
}

/// Returns the icon used for a subspace kind.
fn subspace_icon(subspace_kind: SubspaceKind) -> &'static re_ui::Icon {
    match subspace_kind {
        SubspaceKind::TwoD => &re_ui::icons::VIEW_2D,
        SubspaceKind::ThreeD => &re_ui::icons::VIEW_3D,
    }
}

/// Returns the short label used for a subspace kind.
fn subspace_kind_label(subspace_kind: SubspaceKind) -> &'static str {
    match subspace_kind {
        SubspaceKind::TwoD => "2D",
        SubspaceKind::ThreeD => "3D",
    }
}

/// Draws the edge hover tooltip for an individual transform.
fn edge_tooltip_ui(ui: &mut egui::Ui, model: &Model, edge: &Edge) {
    list_item::list_item_scope(ui, "transform_edge_hover", |ui| {
        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Parent")
                .min_desired_width(FRAME_PROPERTY_MIN_WIDTH)
                .value_text(model.frame_label(edge.parent)),
        );
        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Child")
                .min_desired_width(FRAME_PROPERTY_MIN_WIDTH)
                .value_text(model.frame_label(edge.child)),
        );
        ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Time").value_fn(
            |ui, _| {
                ui.horizontal(|ui| {
                    ui.small_icon(edge_time_icon(edge.time), Some(ui.tokens().text_subdued));
                    ui.label(edge_time_label(edge.time));
                });
            },
        ));
        match &edge.source {
            EdgeSource::ImplicitHierarchy => {}
            EdgeSource::Transform {
                entity_path,
                transform,
            } => {
                edge_source_tooltip_ui(
                    ui,
                    entity_path,
                    Some(transform.transform.translation),
                    transform.transform.matrix3.to_cols_array(),
                );
            }
            EdgeSource::Pinhole {
                entity_path,
                pinhole,
            } => {
                edge_source_tooltip_ui(
                    ui,
                    entity_path,
                    None,
                    pinhole.image_from_camera.0.0.map(f64::from),
                );
            }
        }
    });
}

/// Draws transform source details in an edge hover tooltip.
fn edge_source_tooltip_ui(
    ui: &mut egui::Ui,
    entity_path: &re_log_types::EntityPath,
    translation: Option<glam::DVec3>,
    matrix_cols: [f64; 9],
) {
    ui.separator();
    ui.list_item_flat_noninteractive(
        list_item::PropertyContent::new("Entity").value_text(entity_path.ui_string()),
    );
    if let Some(translation) = translation {
        ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Translation").value_fn(
            |ui, _| {
                ui.monospace(format_dvec3(translation));
            },
        ));
    }
    ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Matrix 3x3").value_fn(
        |ui, _| {
            matrix3x3_ui(ui, matrix_cols);
        },
    ));
}

/// Draws the hover tooltip for a shared fork path.
fn shared_fork_tooltip_ui(ui: &mut egui::Ui, model: &Model, parent: TransformFrameIdHash) {
    let num_transforms = model.num_children(parent);
    ui.label(format!(
        "{} transform{}",
        num_transforms,
        if num_transforms == 1 { "" } else { "s" }
    ));
    ui.colored_label(
        ui.tokens().text_subdued,
        "Hover a terminal segment or icon to inspect an individual transform.",
    );
}

/// Formats a translation vector for tooltip display.
fn format_dvec3(vec: glam::DVec3) -> String {
    format!(
        "[{}, {}, {}]",
        re_format::format_f64(vec.x),
        re_format::format_f64(vec.y),
        re_format::format_f64(vec.z)
    )
}

/// Draws a compact 3x3 matrix in the edge hover tooltip.
fn matrix3x3_ui(ui: &mut egui::Ui, matrix_cols: [f64; 9]) {
    egui::Grid::new("transform_edge_matrix3x3")
        .num_columns(3)
        .spacing(egui::vec2(8.0, 2.0))
        .show(ui, |ui| {
            for row in 0..3 {
                for col in 0..3 {
                    ui.monospace(re_format::format_f64(matrix_cols[row + col * 3]));
                }
                ui.end_row();
            }
        });
}

/// Returns the short label used for an edge time.
fn edge_time_label(time: TimeInt) -> &'static str {
    if time.is_static() {
        "static"
    } else {
        "temporal"
    }
}
