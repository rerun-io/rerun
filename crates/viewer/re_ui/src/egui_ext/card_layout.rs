use egui::{Color32, Frame, NumExt as _, Ui};

use super::response_ext::ResponseExt as _;

/// Per-item configuration for [`CardLayout`].
pub struct CardLayoutItem {
    /// Frame drawn around this card. If `None`, uses the [`CardLayout`]'s default frame.
    pub frame: Option<Frame>,
    pub min_width: f32,
}

/// A virtualized card layout that arranges items in a responsive grid.
///
/// Items are laid out left-to-right, wrapping into rows. Each row is as wide as the
/// available space, with items growing proportionally from their `min_width`.
/// Only rows that intersect the visible (clip) rectangle are rendered;
/// row heights are measured each frame and cached for the next frame's layout.
pub struct CardLayout {
    items: Vec<CardLayoutItem>,
    default_frame: Frame,
    hover_overlay: Option<Color32>,
    all_rows_use_available_width: bool,
}

/// Pre-computed assignment of items to a single row.
struct RowAssignment {
    first_item: usize,
    num_items: usize,
    total_width: f32,
}

#[derive(Default, Debug, Clone)]
struct RowStats {
    max_height: f32,
}

impl CardLayout {
    /// Create a layout where every card has the same minimum width and frame.
    pub fn uniform(num_items: usize, min_width: f32, frame: Frame) -> Self {
        Self {
            items: (0..num_items)
                .map(|_| CardLayoutItem {
                    min_width,
                    frame: None,
                })
                .collect(),
            default_frame: frame,
            hover_overlay: None,
            all_rows_use_available_width: true,
        }
    }

    /// Create a layout with per-item configuration and a shared default frame.
    pub fn new(items: Vec<CardLayoutItem>, default_frame: Frame) -> Self {
        Self {
            items,
            default_frame,
            hover_overlay: None,
            all_rows_use_available_width: true,
        }
    }

    /// Whether all rows stretch to fill the available width (default: `true`).
    ///
    /// When set to `false`, cards on the last row keep the same width
    /// they would have on a full row.
    pub fn all_rows_use_available_width(mut self, value: bool) -> Self {
        self.all_rows_use_available_width = value;
        self
    }

    /// Set an overlay color painted on top of hovered cards.
    ///
    /// The presence of this color enables hover interaction on cards.
    pub fn hover_overlay(mut self, color: Color32) -> Self {
        self.hover_overlay = Some(color);
        self
    }

    pub fn show(self, ui: &mut Ui, mut show_item: impl FnMut(&mut Ui, usize)) {
        let Self {
            items,
            default_frame,
            hover_overlay,
            all_rows_use_available_width,
        } = self;

        if items.is_empty() {
            return;
        }

        re_tracing::profile_function!();

        let available_width = ui.available_width();
        let item_spacing = ui.spacing().item_spacing;

        // Assign items to rows based on available width.
        let rows = Self::assign_items_to_rows(&items, available_width, item_spacing.x);

        // Read cached row heights from previous frame.
        // For rows without cached data, use the nearest known row height (or 100 as a last resort).
        let stats_id = ui.id().with("card_layout_row");
        let mut last_known_height = 100.0;
        let row_heights: Vec<f32> = (0..rows.len())
            .map(|i| {
                let h = ui
                    .data(|d| d.get_temp::<RowStats>(stats_id.with(i)))
                    .map_or(last_known_height, |s| s.max_height);
                last_known_height = h;
                h
            })
            .collect();

        // Reserve full content height so the scrollbar is correct.
        let total_height =
            row_heights.iter().sum::<f32>() + item_spacing.y * rows.len().saturating_sub(1) as f32;
        let (full_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, total_height.at_least(0.0)),
            egui::Sense::hover(),
        );

        let visible = ui.clip_rect();
        let mut row_y = full_rect.min.y;

        for (row_idx, (row, row_height)) in rows.iter().zip(row_heights.iter()).enumerate() {
            // Skip rows outside the visible area.
            if row_y > visible.max.y {
                break; // Done!
            }
            if row_y + row_height < visible.min.y {
                row_y += row_height + item_spacing.y;
                ui.skip_ahead_auto_ids(row.num_items);
                continue;
            }

            let gap_space = item_spacing.x * (row.num_items - 1) as f32;
            let gap_space_item = gap_space / row.num_items as f32;
            let is_last_row = row_idx + 1 == rows.len();
            let item_growth = if !all_rows_use_available_width && is_last_row && rows.len() > 1 {
                // Use the first row's growth factor so last-row cards
                // stay the same width as cards on full rows.
                available_width / rows[0].total_width
            } else {
                available_width / row.total_width
            };

            let mut card_x = full_rect.min.x;
            let mut new_row_stats = RowStats::default();

            for i in 0..row.num_items {
                let item = &items[row.first_item + i];
                let frame = item.frame.unwrap_or(default_frame);
                let frame_margin = frame.inner_margin.sum();
                let card_width =
                    (item_growth * item.min_width - gap_space_item).at_most(available_width);

                let card_rect = egui::Rect::from_min_size(
                    egui::pos2(card_x, row_y),
                    egui::vec2(card_width, *row_height),
                );

                let mut child_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(card_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );

                let mut content_height = 0.0;
                let frame_response = frame.show(&mut child_ui, |ui| {
                    ui.set_width((card_width - frame_margin.x).at_most(ui.available_width()));
                    show_item(ui, row.first_item + i);

                    content_height = ui.min_size().y;
                    ui.set_height((row_height - frame_margin.y).at_least(0.0));
                });

                // Paint a hover overlay on top of the card.
                // Use `container_hovered` rather than `hovered()` so the overlay
                // persists even when child widgets (e.g. flag buttons) consume clicks.
                if let Some(overlay) = hover_overlay
                    && frame_response.response.container_hovered()
                {
                    child_ui.painter().rect_filled(
                        frame_response.response.rect,
                        frame.corner_radius,
                        overlay,
                    );
                }

                new_row_stats.max_height = new_row_stats
                    .max_height
                    .max(content_height + frame_margin.y);
                card_x += card_width + item_spacing.x;
            }

            ui.data_mut(|d| d.insert_temp(stats_id.with(row_idx), new_row_stats));

            row_y += row_height + item_spacing.y;
        }
    }

    fn assign_items_to_rows(
        items: &[CardLayoutItem],
        available_width: f32,
        item_spacing: f32,
    ) -> Vec<RowAssignment> {
        let mut idx = 0;
        std::iter::from_fn(|| {
            if idx >= items.len() {
                return None;
            }
            let first_item = idx;
            let mut total_width = 0.0;
            let mut count = 0;
            while idx < items.len() {
                let spacing = item_spacing * (count + 1) as f32; // +1 to account for spacing to the right of the card.
                let needed = total_width + items[idx].min_width + spacing;
                if needed > available_width && count > 0 {
                    break;
                }
                total_width += items[idx].min_width;
                count += 1;
                idx += 1;
            }
            Some(RowAssignment {
                first_item,
                num_items: idx - first_item,
                total_width,
            })
        })
        .collect()
    }
}
