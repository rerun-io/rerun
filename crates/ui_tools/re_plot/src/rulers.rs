//! Rulers with tick values and axis names along the top and left edge of a 2D view.

use std::sync::Arc;

use egui::emath::{GuiRounding as _, RectTransform, remap_clamp};
use egui::{
    Align2, Color32, FontId, Galley, Painter, Pos2, Rangef, Rect, Stroke, TextStyle, Vec2, pos2,
    vec2,
};
use egui_plot::{GridInput, GridMark};

/// Smallest distance between two ticks, in ui points.
const MIN_TICK_SPACING: f32 = 8.0;

/// Tick spacing at which a tick has its full length and color, in ui points.
const FULL_TICK_SPACING: f32 = 80.0;

/// Range of free space between two neighboring tick labels over which the labels fade in,
/// in ui points.
const LABEL_GAP: Rangef = Rangef {
    min: 8.0,
    max: 16.0,
};

/// Length of a tick at full strength, in ui points.
const TICK_LENGTH: f32 = 4.0;

/// Space between the end of a tick and its label on the vertical axis, in ui points.
const GAP: f32 = 2.0;

/// Space between an axis name and the edge of the view, in ui points.
const NAME_MARGIN: f32 = 4.0;

/// Free space between an axis name and a tick label at which the label has full room,
/// in ui points.
const NAME_GAP: f32 = 8.0;

/// Distance between a tick label and the nearest end of its axis at which the label has full
/// room, in ui points.
const EDGE_GAP: f32 = 8.0;

/// Largest number of characters in a tick label.
///
/// Longer tick values are shown in scientific notation.
const MAX_TICK_LABEL_CHARS: usize = 9;

/// Strength at which a tick label starts to fade in.
///
/// Up to this strength, a label of the vertical axis widens the column of tick labels toward its
/// own width. From this strength on, the column fits the label and the label fades in.
const LABEL_FADE_START: f32 = 0.5;

/// Smallest change of the width of the column of tick labels that is stored, in ui points.
const MIN_COLUMN_CHANGE: f32 = 1.0 / 32.0;

/// Largest width of an axis name, in ui points.
///
/// Longer names are cut off with `…`.
const MAX_NAME_WIDTH: f32 = 100.0;

/// Rulers along the top and left edge of a 2D view.
pub struct Rulers {
    /// Where the width of the column of tick labels of the vertical axis is stored between
    /// frames.
    id: egui::Id,

    /// Name shown at the right end of the horizontal axis.
    x_name: String,

    /// Name shown at the bottom end of the vertical axis.
    y_name: String,
}

impl Rulers {
    /// Line breaks in the axis names are shown as spaces.
    /// Axis names wider than `MAX_NAME_WIDTH` are cut off with `…`.
    pub fn new(id: egui::Id, x_name: &str, y_name: &str) -> Self {
        Self {
            id,
            x_name: single_line(x_name),
            y_name: single_line(y_name),
        }
    }

    /// Splits `rect` into the rulers and the part that shows the scene.
    ///
    /// The column left of the scene has the width `column_width` gave in the previous frame.
    ///
    /// Returns `None` if `rect` has no room for the rulers.
    pub fn layout(&self, ui: &egui::Ui, rect: Rect) -> Option<RulersLayout> {
        let painter = ui.painter();
        let font_id = TextStyle::Body.resolve(ui.style());
        let layout_name = |name: &str| {
            (!name.is_empty()).then(|| {
                // The text has no color of its own, so the color passed when painting it applies.
                let mut job = egui::text::LayoutJob::simple_singleline(
                    name.to_owned(),
                    font_id.clone(),
                    Color32::PLACEHOLDER,
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(MAX_NAME_WIDTH);
                painter.layout_job(job)
            })
        };
        let x_name = layout_name(&self.x_name);
        let y_name = layout_name(&self.y_name);

        let label_width = ui
            .data(|data| data.get_temp::<f32>(self.id))
            .unwrap_or_default();
        // The vertical axis name sits at the bottom of the column of tick labels.
        let y_name_width = y_name.as_ref().map_or(0.0, |y_name| y_name.size().x);
        let left = label_width.max(y_name_width) + GAP + TICK_LENGTH;
        let top = TICK_LENGTH + ui.text_style_height(&TextStyle::Body);

        // `egui::Ui::allocate_rect` rounds the rect it allocates, so the rulers are painted
        // along the same rounded rect the scene gets.
        let content_rect = Rect::from_min_max(
            pos2(rect.left() + left, rect.top() + top),
            rect.right_bottom(),
        )
        .round_ui();

        content_rect.is_positive().then(|| RulersLayout {
            id: self.id,
            rect,
            content_rect,
            x_name,
            y_name,
        })
    }
}

/// Rulers laid out by [`Rulers::layout`] for one frame.
pub struct RulersLayout {
    /// Where the width of the column of tick labels of the vertical axis is stored between
    /// frames.
    id: egui::Id,

    /// The whole area of the rulers and the scene.
    rect: Rect,

    /// The part of `rect` that shows the scene.
    content_rect: Rect,

    /// Name shown at the right end of the horizontal axis.
    x_name: Option<Arc<Galley>>,

    /// Name shown at the bottom end of the vertical axis.
    y_name: Option<Arc<Galley>>,
}

impl RulersLayout {
    /// The part of the laid out rect that shows the scene, next to the rulers.
    pub fn content_rect(&self) -> Rect {
        self.content_rect
    }

    /// Paints the rulers around the scene that `ui_from_scene` shows in [`Self::content_rect`].
    pub fn paint(&self, ui: &egui::Ui, ui_from_scene: &RectTransform) {
        let painter = ui.painter().with_clip_rect(self.rect);
        let font_id = TextStyle::Body.resolve(ui.style());
        let text_color = ui.visuals().text_color();
        let line_color = ui.visuals().widgets.noninteractive.fg_stroke.color;

        // Width and strength of each shown tick label of the vertical axis.
        let mut y_labels: Vec<(f32, f32)> = Vec::new();
        for axis in [Axis::X, Axis::Y] {
            painter.line_segment(axis.line(self.content_rect), Stroke::new(1.0, line_color));
            for tick in self.ticks(&painter, &font_id, ui_from_scene, axis) {
                let start = axis.point(self.content_rect, tick.pos);
                let end = start + axis.outward() * TICK_LENGTH * tick.strength;
                painter.line_segment(
                    [start, end],
                    Stroke::new(1.0, line_color.gamma_multiply(tick.strength.sqrt())),
                );

                if let Some(label) = tick.label {
                    if axis == Axis::Y {
                        y_labels.push((label.rect.width(), label.strength));
                    }
                    label.paint(&painter, text_color);
                }
            }
        }

        for (name, name_rect) in [
            (&self.x_name, self.name_rect(Axis::X)),
            (&self.y_name, self.name_rect(Axis::Y)),
        ] {
            if let (Some(name), Some(name_rect)) = (name, name_rect) {
                painter.galley(name_rect.min, Arc::clone(name), text_color);
            }
        }

        self.store_column_width(ui, column_width(&y_labels));
    }

    /// Returns the ticks of `axis` that lie within the scene.
    fn ticks(
        &self,
        painter: &Painter,
        font_id: &FontId,
        ui_from_scene: &RectTransform,
        axis: Axis,
    ) -> Vec<Tick> {
        let index = axis as usize;
        let axis_range = axis.range(self.content_rect);
        let scene_range = axis.range(ui_from_scene.inverse().transform_rect(self.content_rect));
        let points_per_unit = ui_from_scene.scale()[index].abs();

        let (label_align, label_distance) = axis.label_anchor();
        let name_rect = self.name_rect(axis);

        // Each mark with its position and its label, if the label has any room.
        // The strength of each label is its room for now.
        let marks: Vec<(f32, GridMark, Option<TickLabel>)> =
            grid_marks(scene_range, points_per_unit)
                .into_iter()
                .filter_map(|mark| {
                    let pos =
                        ui_from_scene.transform_pos(axis.scene_point(mark.value as f32))[index];
                    axis_range.contains(pos).then(|| {
                        let text = crate::format_axis_tick_with_max_chars(
                            mark.value,
                            mark.step_size,
                            MAX_TICK_LABEL_CHARS,
                        );
                        // The text has no color of its own, so the color passed when painting it
                        // applies.
                        let galley =
                            painter.layout_no_wrap(text, font_id.clone(), Color32::PLACEHOLDER);
                        let anchor =
                            axis.point(self.content_rect, pos) + axis.outward() * label_distance;
                        let label_rect = label_align.anchor_size(anchor, galley.size());
                        let room = label_room(axis, label_rect, axis_range, name_rect);
                        let label = (0.0 < room).then_some(TickLabel {
                            rect: label_rect,
                            galley,
                            strength: room,
                        });
                        (pos, mark, label)
                    })
                })
                .collect();

        // Largest size along the axis among the labels with room, for each step size.
        // Each size is scaled by the room of its label, so the largest size changes
        // continuously as labels move in at the ends of the axis.
        // The grid spacer uses at most three step sizes.
        let mut largest_by_step: Vec<(f64, f32)> = Vec::new();
        for (_, mark, label) in &marks {
            let Some(label) = label else {
                continue;
            };
            let size = label.rect.size()[index] * label.strength;
            if let Some((_, largest)) = largest_by_step
                .iter_mut()
                .find(|(step_size, _)| *step_size == mark.step_size)
            {
                *largest = largest.max(size);
            } else {
                largest_by_step.push((mark.step_size, size));
            }
        }

        // Largest size along the axis among the labels of the ticks that are at least
        // `step_size` apart.
        let largest_label = |step_size: f64| {
            largest_by_step
                .iter()
                .filter(|(step, _)| step_size <= *step)
                .map(|(_, size)| *size)
                .fold(0.0, f32::max)
        };

        marks
            .into_iter()
            .map(|(pos, mark, label)| {
                let spacing = mark.step_size as f32 * points_per_unit;
                let free_space = spacing - largest_label(mark.step_size);
                let label = label
                    .map(|label| TickLabel {
                        strength: label
                            .strength
                            .min(remap_clamp(free_space, LABEL_GAP, 0.0..=1.0)),
                        ..label
                    })
                    .filter(|label| 0.0 < label.strength);
                Tick {
                    pos,
                    // Depends on the distance to the next tick of the same step.
                    strength: remap_clamp(spacing, MIN_TICK_SPACING..=FULL_TICK_SPACING, 0.0..=1.0),
                    label,
                }
            })
            .collect()
    }

    /// Where the name of `axis` is shown, at the far end of the axis in the row or column of its
    /// tick labels.
    fn name_rect(&self, axis: Axis) -> Option<Rect> {
        let content_rect = self.content_rect;
        match axis {
            Axis::X => self.x_name.as_ref().map(|x_name| {
                Align2::RIGHT_BOTTOM.anchor_size(
                    pos2(
                        content_rect.right() - NAME_MARGIN,
                        content_rect.top() - TICK_LENGTH,
                    ),
                    x_name.size(),
                )
            }),
            Axis::Y => self.y_name.as_ref().map(|y_name| {
                Align2::RIGHT_BOTTOM.anchor_size(
                    pos2(
                        content_rect.left() - TICK_LENGTH - GAP,
                        content_rect.bottom() - NAME_MARGIN,
                    ),
                    y_name.size(),
                )
            }),
        }
    }

    /// Stores the width of the column of tick labels of the vertical axis for the next frame's
    /// [`Rulers::layout`].
    ///
    /// Changes smaller than [`MIN_COLUMN_CHANGE`] are not stored.
    /// The first stored width discards the frame, any later change requests a repaint.
    fn store_column_width(&self, ui: &egui::Ui, width: f32) {
        let previous = ui.data(|data| data.get_temp::<f32>(self.id));
        if previous.is_some_and(|previous| (width - previous).abs() < MIN_COLUMN_CHANGE) {
            return;
        }
        ui.data_mut(|data| data.insert_temp(self.id, width));
        if previous.is_none() {
            ui.ctx()
                .request_discard("The column of ruler tick labels has no width yet");
        } else {
            ui.request_repaint();
        }
    }
}

/// Width of the vertical tick label column, from the `(width, strength)` of each label.
///
/// Fading labels count less, so the width changes smoothly while zooming.
fn column_width(labels: &[(f32, f32)]) -> f32 {
    let share = |strength: f32| remap_clamp(strength, 0.0..=LABEL_FADE_START, 0.0..=1.0);

    // The need of each label measured from zero, which the other labels widen from.
    let own_needs: Vec<f32> = labels
        .iter()
        .map(|&(width, strength)| width * share(strength))
        .collect();

    labels
        .iter()
        .enumerate()
        .map(|(index, &(width, strength))| {
            let others = own_needs
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, need)| *need)
                .fold(0.0, f32::max);
            egui::lerp(others.min(width)..=width, share(strength))
        })
        .fold(0.0, f32::max)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    /// The horizontal axis along the top edge.
    X = 0,

    /// The vertical axis along the left edge.
    Y = 1,
}

impl Axis {
    /// The extent of `rect` along the axis.
    fn range(self, rect: Rect) -> Rangef {
        match self {
            Self::X => rect.x_range(),
            Self::Y => rect.y_range(),
        }
    }

    /// The axis line along the edge of `content_rect`.
    fn line(self, content_rect: Rect) -> [Pos2; 2] {
        match self {
            Self::X => [content_rect.left_top(), content_rect.right_top()],
            Self::Y => [content_rect.left_top(), content_rect.left_bottom()],
        }
    }

    /// The point on the axis line at `pos` along the axis, in ui points.
    fn point(self, content_rect: Rect, pos: f32) -> Pos2 {
        match self {
            Self::X => pos2(pos, content_rect.top()),
            Self::Y => pos2(content_rect.left(), pos),
        }
    }

    /// The point in scene coordinates at `value` along the axis.
    fn scene_point(self, value: f32) -> Pos2 {
        match self {
            Self::X => pos2(value, 0.0),
            Self::Y => pos2(0.0, value),
        }
    }

    /// The direction from the axis line toward its ticks and labels.
    fn outward(self) -> Vec2 {
        match self {
            Self::X => vec2(0.0, -1.0),
            Self::Y => vec2(-1.0, 0.0),
        }
    }

    /// How a tick label is anchored, and its distance from the axis line along
    /// [`Self::outward`].
    fn label_anchor(self) -> (Align2, f32) {
        match self {
            // The row of text has space below the digits, so the labels need no gap above the
            // ticks.
            Self::X => (Align2::CENTER_BOTTOM, TICK_LENGTH),
            Self::Y => (Align2::RIGHT_CENTER, TICK_LENGTH + GAP),
        }
    }
}

/// A tick on one of the axes.
struct Tick {
    /// Where the tick lies along its axis, in ui points.
    pos: f32,

    /// How long and how strong the tick is drawn, from 0 to 1.
    strength: f32,

    /// The tick label, if there is room to show it.
    label: Option<TickLabel>,
}

/// The value of a tick, laid out next to the tick.
struct TickLabel {
    rect: Rect,

    galley: Arc<Galley>,

    /// How much room the label has, from 0 to 1.
    ///
    /// The label is drawn from [`LABEL_FADE_START`] on.
    strength: f32,
}

impl TickLabel {
    fn paint(self, painter: &Painter, text_color: Color32) {
        let opacity = remap_clamp(self.strength, LABEL_FADE_START..=1.0, 0.0..=1.0);
        if 0.0 < opacity {
            painter.galley(
                self.rect.min,
                self.galley,
                text_color.gamma_multiply(opacity.sqrt()),
            );
        }
    }
}

/// How much room a tick label of `axis` at `label_rect` has, from 0 to 1.
///
/// A label has no room until it lies within `axis_range`, the extent of the axis,
/// so no label reaches into the corner between the two axes.
/// It has full room [`EDGE_GAP`] inside the ends of the axis and [`NAME_GAP`] away from the
/// axis name at `name_rect`, and the room grows continuously in between.
fn label_room(axis: Axis, label_rect: Rect, axis_range: Rangef, name_rect: Option<Rect>) -> f32 {
    let label_range = axis.range(label_rect);
    let edge_distance = f32::min(
        label_range.min - axis_range.min,
        axis_range.max - label_range.max,
    );
    let name_distance = name_rect.map_or(f32::INFINITY, |name_rect| {
        let name_range = axis.range(name_rect);
        f32::max(
            name_range.min - label_range.max,
            label_range.min - name_range.max,
        )
    });
    f32::min(
        remap_clamp(edge_distance, 0.0..=EDGE_GAP, 0.0..=1.0),
        remap_clamp(name_distance, 0.0..=NAME_GAP, 0.0..=1.0),
    )
}

/// Returns `text` with each line break replaced by a space.
fn single_line(text: &str) -> String {
    text.lines().collect::<Vec<_>>().join(" ")
}

/// Returns the ticks within `range`, whose ends can be in any order.
///
/// Returns no ticks if `range` or `points_per_unit` is not finite, or `points_per_unit` is not
/// positive.
fn grid_marks(range: Rangef, points_per_unit: f32) -> Vec<GridMark> {
    if !(range.min.is_finite()
        && range.max.is_finite()
        && points_per_unit.is_finite()
        && 0.0 < points_per_unit)
    {
        return Vec::new();
    }
    let spacer = egui_plot::log_grid_spacer(10);
    spacer(GridInput {
        bounds: (
            range.min.min(range.max) as f64,
            range.min.max(range.max) as f64,
        ),
        base_step_size: (MIN_TICK_SPACING / points_per_unit) as f64,
    })
}
