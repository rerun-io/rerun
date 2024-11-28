use std::sync::Arc;

use egui::{
    Align2, Color32, FontId, FontSelection, Frame, Galley, Painter, Pos2, Rect, Response, RichText,
    Sense, Shape, Stroke, TextWrapMode, Ui, UiBuilder, Vec2, WidgetText,
};
use re_chunk::EntityPath;
use re_types::ArrowString;
use re_viewer_context::{
    HoverHighlight, InteractionHighlight, SelectionHighlight, SpaceViewHighlights,
};

use crate::visualizers::Label;

// Sorry for the pun, could not resist 😎.
// On a serious note, is there no other way to create a `Sense` that does nothing?
const NON_SENSE: Sense = Sense {
    click: false,
    drag: false,
    focusable: false,
};

pub enum DrawableLabel {
    Circle(CircleLabel),
    Text(TextLabel),
}

impl DrawableLabel {
    pub fn from_label(ui: &Ui, label: &Label) -> Self {
        match label {
            &Label::Circle { radius, color } => Self::circle(radius, color),
            Label::Text { text, color } => Self::text(ui, text, *color),
        }
    }
}

pub struct TextLabel {
    frame: Frame,
    galley: Arc<Galley>,
}

pub struct CircleLabel {
    radius: f32,
    color: Option<Color32>,
}

impl DrawableLabel {
    pub fn size(&self) -> Vec2 {
        match self {
            Self::Circle(CircleLabel { radius, .. }) => Vec2::splat(radius * 2.0),
            Self::Text(TextLabel { galley, frame }) => {
                frame.inner_margin.sum() + galley.size() + Vec2::splat(frame.stroke.width * 2.0)
            }
        }
    }

    pub fn circle(radius: f32, color: Option<Color32>) -> Self {
        Self::Circle(CircleLabel { radius, color })
    }

    pub fn implicit_circle() -> Self {
        Self::Circle(CircleLabel {
            radius: 4.0,
            color: None,
        })
    }

    pub fn text(ui: &Ui, text: &ArrowString, color: Option<Color32>) -> Self {
        let galley = WidgetText::from(
            RichText::new(text.to_string())
                .color(color.unwrap_or_else(|| ui.style().visuals.text_color())),
        )
        .into_galley(
            ui,
            Some(TextWrapMode::Extend),
            f32::INFINITY,
            FontSelection::Default,
        );

        let frame = Frame::default()
            .inner_margin(Vec2::new(6.0, 4.0))
            .fill(ui.style().visuals.widgets.noninteractive.bg_fill)
            .stroke(Stroke::new(1.0, ui.style().visuals.text_color()));

        Self::Text(TextLabel { frame, galley })
    }
}

fn draw_circle_label(
    ui: &mut Ui,
    label: &CircleLabel,
    _highlight: InteractionHighlight,
) -> Response {
    let &CircleLabel { radius, color } = label;
    let (resp, painter) = ui.allocate_painter(Vec2::splat(radius * 2.0), Sense::click());
    painter.circle(
        resp.rect.center(),
        radius,
        color.unwrap_or_else(|| ui.style().visuals.text_color()),
        Stroke::NONE,
    );
    resp
}

fn draw_text_label(ui: &mut Ui, label: &TextLabel, highlight: InteractionHighlight) -> Response {
    let TextLabel { galley, frame } = label;
    let visuals = &ui.style().visuals;

    let bg = match highlight.hover {
        HoverHighlight::None => visuals.widgets.noninteractive.bg_fill,
        HoverHighlight::Hovered => visuals.widgets.hovered.bg_fill,
    };

    let stroke = match highlight.selection {
        SelectionHighlight::Selection => visuals.selection.stroke,
        _ => Stroke::new(1.0, visuals.text_color()),
    };

    frame
        .stroke(stroke)
        .fill(bg)
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(galley.clone())
                    .selectable(false)
                    .sense(Sense::click()),
            )
        })
        .inner
}

/// Draws a node at the given position.
pub fn draw_node(
    ui: &mut Ui,
    center: Pos2,
    node: &DrawableLabel,
    highlight: InteractionHighlight,
) -> Response {
    let builder = UiBuilder::new().max_rect(Rect::from_center_size(center, node.size()));
    let mut node_ui = ui.new_child(builder);

    // TODO(grtlr): handle highlights

    match node {
        DrawableLabel::Circle(label) => draw_circle_label(&mut node_ui, label, highlight),
        DrawableLabel::Text(label) => draw_text_label(&mut node_ui, label, highlight),
    }
}

/// Draws a bounding box, as well as a basic coordinate system.
pub fn draw_debug(ui: &Ui, world_bounding_rect: Rect) {
    let painter = ui.painter();

    // Paint coordinate system at the world origin
    let origin = Pos2::new(0.0, 0.0);
    let x_axis = Pos2::new(100.0, 0.0);
    let y_axis = Pos2::new(0.0, 100.0);

    painter.line_segment([origin, x_axis], Stroke::new(1.0, Color32::RED));
    painter.line_segment([origin, y_axis], Stroke::new(1.0, Color32::GREEN));

    if world_bounding_rect.is_positive() {
        painter.rect(
            world_bounding_rect,
            0.0,
            Color32::from_rgba_unmultiplied(255, 0, 255, 8),
            Stroke::new(1.0, Color32::from_rgb(255, 0, 255)),
        );
    }
}

/// Helper function to draw an arrow at the end of the edge
fn draw_arrow(painter: &Painter, tip: Pos2, direction: Vec2, color: Color32) {
    let arrow_size = 10.0; // Adjust size as needed
    let perpendicular = Vec2::new(-direction.y, direction.x) * 0.5 * arrow_size;

    let p1 = tip - direction * arrow_size + perpendicular;
    let p2 = tip - direction * arrow_size - perpendicular;

    // Draw a filled triangle for the arrow
    painter.add(Shape::convex_polygon(
        vec![tip, p1, p2],
        color,
        Stroke::NONE,
    ));
}

/// Draws an edge between two points, optionally with an arrow at the target point.
pub fn draw_edge(ui: &mut Ui, points: [Pos2; 2], show_arrow: bool) -> Response {
    let fg = ui.style().visuals.text_color();

    let rect = Rect::from_points(&points);
    let painter = ui.painter();
    painter.line_segment(points, Stroke::new(1.0, fg));

    // Calculate direction vector from source to target
    let direction = (points[1] - points[0]).normalized();

    // Conditionally draw an arrow at the target point
    if show_arrow {
        draw_arrow(painter, points[1], direction, fg);
    }

    // We can add interactions in the future, for now we simply allocate the
    // rect, so that bounding boxes are computed correctly.
    ui.allocate_rect(rect, NON_SENSE)
}

pub fn draw_entity_rect(
    ui: &mut Ui,
    rect: Rect,
    entity_path: &EntityPath,
    highlights: &SpaceViewHighlights,
) -> Response {
    let color = if highlights
        .entity_outline_mask(entity_path.hash())
        .overall
        .is_some()
    {
        ui.ctx().style().visuals.text_color()
    } else {
        ui.ctx()
            .style()
            .visuals
            .gray_out(ui.ctx().style().visuals.text_color())
    };

    let padded = rect.expand(10.0);

    ui.painter()
        .rect(padded, 0.0, Color32::TRANSPARENT, Stroke::new(1.0, color));

    ui.painter().text(
        padded.left_top(),
        Align2::LEFT_BOTTOM,
        entity_path.to_string(),
        FontId {
            size: 12.0,
            family: Default::default(),
        },
        color,
    );

    ui.allocate_rect(rect, Sense::click())
}
