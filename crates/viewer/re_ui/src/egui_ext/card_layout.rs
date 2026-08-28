use egui::{Color32, Frame, NumExt as _, Stroke, Ui};

/// Per-item configuration for [`CardLayout`].
pub struct CardLayoutItem {
    /// Frame drawn around this card. If `None`, uses the [`CardLayout`]'s default frame.
    pub frame: Option<Frame>,
    pub min_width: f32,

    /// Whether a click on this card counts as picking it.
    /// If `None`, follows [`CardLayout::clickable`].
    ///
    /// A card set to `Some(false)` gets no hover highlight and no pointing hand, and
    /// [`CardLayout::show`] never returns its index.
    pub clickable: Option<bool>,
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
    hover_fill: Option<Color32>,
    hover_stroke: Option<Stroke>,
    all_rows_use_available_width: bool,
    clickable: bool,
}

/// Pre-computed assignment of items to a single row.
struct RowAssignment {
    items: re_span::Span<usize>,
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
                    clickable: None,
                })
                .collect(),
            default_frame: frame,
            hover_fill: None,
            hover_stroke: None,
            all_rows_use_available_width: true,
            clickable: false,
        }
    }

    /// Create a layout with per-item configuration and a shared default frame.
    pub fn new(items: Vec<CardLayoutItem>, default_frame: Frame) -> Self {
        Self {
            items,
            default_frame,
            hover_fill: None,
            hover_stroke: None,
            all_rows_use_available_width: true,
            clickable: false,
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

    /// Set a fill color used for hovered cards (replaces the default frame fill).
    ///
    /// The card's frame fill is swapped to this color when the pointer is over the card.
    pub fn hover_fill(mut self, color: Color32) -> Self {
        self.hover_fill = Some(color);
        self
    }

    /// Set the outline used for hovered cards, replacing the default frame stroke.
    ///
    /// Marking hover by the outline alone leaves the fill alone, so the text on a card keeps the
    /// same contrast under the pointer.
    pub fn hover_stroke(mut self, stroke: Stroke) -> Self {
        self.hover_stroke = Some(stroke);
        self
    }

    /// Let a click anywhere on a card count as picking it, see [`Self::show`].
    ///
    /// Applies to every card that leaves [`CardLayoutItem::clickable`] unset.
    ///
    /// A widget inside a card keeps its own click. Selectable text is turned off inside, so a
    /// label doesn't swallow the card's click. [`crate::list_item::ListItem`] does the same.
    pub fn clickable(mut self) -> Self {
        self.clickable = true;
        self
    }

    /// Render the grid.
    ///
    /// `show_item` receives `(ui, item_index, pointer_over)`. The `pointer_over`
    /// flag is `true` when the pointer is over the card, which lets content
    /// (e.g. a flag button) adapt its appearance based on parent hover state.
    /// Returns the index of the clicked card. Which cards take clicks is set by
    /// [`CardLayoutItem::clickable`], falling back to [`Self::clickable`].
    pub fn show(
        self,
        ui: &mut Ui,
        mut show_item: impl FnMut(&mut Ui, usize, bool),
    ) -> Option<usize> {
        let Self {
            items,
            default_frame,
            hover_fill,
            hover_stroke,
            all_rows_use_available_width,
            clickable,
        } = self;

        if items.is_empty() {
            return None;
        }

        let mut clicked = None;

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

        for (row_idx, (row, row_height)) in std::iter::zip(&rows, &row_heights).enumerate() {
            // Skip rows outside the visible area.
            if row_y > visible.max.y {
                break; // Done!
            }
            if row_y + row_height < visible.min.y {
                row_y += row_height + item_spacing.y;
                ui.skip_ahead_auto_ids(row.items.len);
                continue;
            }

            let gap_space = item_spacing.x * (row.items.len - 1) as f32;
            let gap_space_item = gap_space / row.items.len as f32;
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

            for i in row.items {
                let item = &items[i];
                let frame = item.frame.unwrap_or(default_frame);
                let item_clickable = item.clickable.unwrap_or(clickable);
                let frame_margin = frame.inner_margin.sum();
                let card_width =
                    (item_growth * item.min_width - gap_space_item).at_most(available_width);

                let card_rect = egui::Rect::from_min_size(
                    egui::pos2(card_x, row_y),
                    egui::vec2(card_width, *row_height),
                );

                // The card senses the click itself. `UiBuilder::sense` registers it before its
                // content, so a widget inside the card takes the click first.
                //
                // The id is keyed by index so the card keeps it from frame to frame: a click needs
                // the press and the release to land on one id.
                let mut child_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id(ui.id().with(("card", i)))
                        .max_rect(card_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Min))
                        .sense(if item_clickable {
                            egui::Sense::click()
                        } else {
                            egui::Sense::hover()
                        }),
                );

                // The response is read before painting, and its interact rect is clipped, so a card
                // sticking out of the scroll area is only hovered over the visible part.
                let card_response = child_ui.response();
                let pointer_over =
                    card_response.contains_pointer() && child_ui.ctx().dragged_id().is_none();

                // A card marked not clickable is left out: it takes no click, so highlighting it
                // under the pointer would offer an interaction it doesn't have. A layout that
                // senses the click itself leaves `clickable` unset and keeps its highlight.
                let highlight = pointer_over
                    && item.clickable != Some(false)
                    && (hover_fill.is_some() || hover_stroke.is_some());

                let mut frame = frame;
                if highlight {
                    if let Some(fill) = hover_fill {
                        frame = frame.fill(fill);
                    }

                    // An item with its own frame uses that outline to say something about itself,
                    // e.g. that it failed, so hover leaves the outline alone.
                    if let (Some(stroke), None) = (hover_stroke, item.frame) {
                        frame = frame.stroke(stroke);
                    }
                }

                if item_clickable {
                    if card_response.clicked() {
                        clicked = Some(i);
                    }
                    if pointer_over {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Selectable text senses clicks, so a label on a card would swallow the card's
                    // own click. Picking a card takes priority over selecting its text, while
                    // buttons and menus on it still come first. `ListItem` does the same.
                    child_ui.style_mut().interaction.selectable_labels = false;
                }

                let mut content_height = 0.0;
                frame.show(&mut child_ui, |ui| {
                    ui.set_width((card_width - frame_margin.x).at_most(ui.available_width()));
                    show_item(ui, i, pointer_over);

                    content_height = ui.min_size().y;
                    ui.set_height((row_height - frame_margin.y).at_least(0.0));
                });

                new_row_stats.max_height = new_row_stats
                    .max_height
                    .max(content_height + frame_margin.y);
                card_x += card_width + item_spacing.x;
            }

            ui.data_mut(|d| d.insert_temp(stats_id.with(row_idx), new_row_stats));

            row_y += row_height + item_spacing.y;
        }

        clicked
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
                items: re_span::Span::from_start_end(first_item, idx),
                total_width,
            })
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::{CardLayout, CardLayoutItem, Frame};

    #[derive(Default)]
    struct Clicks {
        card: Option<usize>,
        button: usize,
    }

    /// One clickable card holding a label and a button, with `item_clickable` set on the card
    /// itself.
    fn harness(item_clickable: Option<bool>) -> Harness<'static, Clicks> {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(300.0, 120.0))
            .build_ui_state(
                move |ui, clicks| {
                    let items = vec![CardLayoutItem {
                        frame: None,
                        min_width: 260.0,
                        clickable: item_clickable,
                    }];

                    let clicked = CardLayout::new(items, Frame::new().inner_margin(16))
                        .clickable()
                        .show(ui, |ui, _idx, _hovered| {
                            ui.label("An asset");
                            if ui.button("Open").clicked() {
                                clicks.button += 1;
                            }
                        });

                    if let Some(idx) = clicked {
                        clicks.card = Some(idx);
                    }
                },
                Clicks::default(),
            );

        // Twice: the layout caches row heights, so the first frame lays the card out at a guessed
        // height and the second one at its real one.
        harness.run();
        harness.run();
        harness
    }

    /// A button on a card keeps its own click instead of the card taking it.
    #[test]
    fn a_button_on_a_card_keeps_its_click() {
        let mut harness = harness(None);

        harness.get_by_label("Open").click();
        harness.run();

        assert_eq!(harness.state().button, 1);
        assert_eq!(harness.state().card, None);
    }

    /// Clicking a card over something that doesn't take clicks itself picks the card.
    #[test]
    fn clicking_a_card_over_plain_content_picks_the_card() {
        let mut harness = harness(None);

        // A label senses nothing, so the card is the only thing under the pointer that does.
        harness.get_by_label("An asset").click();
        harness.run();

        assert_eq!(harness.state().card, Some(0));
        assert_eq!(harness.state().button, 0);
    }

    /// A card that says it takes no clicks is never reported as picked, whatever the layout says.
    #[test]
    fn a_card_marked_not_clickable_is_never_picked() {
        let mut harness = harness(Some(false));

        harness.get_by_label("An asset").click();
        harness.run();

        assert_eq!(harness.state().card, None);
    }

    #[derive(Default)]
    struct ClippedCard {
        hovered: bool,
        visible_bottom: f32,
    }

    /// A card taller than the scroll area holding it, with the pointer resting below that scroll
    /// area, over the part of the card that is clipped away.
    #[test]
    fn a_card_clipped_by_its_scroll_area_is_not_hovered() {
        const VISIBLE_HEIGHT: f32 = 60.0;
        const CARD_CONTENT_HEIGHT: f32 = 200.0;

        let mut harness = Harness::builder()
            .with_size(egui::vec2(300.0, 400.0))
            .build_ui_state(
                |ui, state: &mut ClippedCard| {
                    let scroll = egui::ScrollArea::vertical()
                        .max_height(VISIBLE_HEIGHT)
                        .show(ui, |ui| {
                            let items = vec![CardLayoutItem {
                                frame: None,
                                min_width: 260.0,
                                clickable: None,
                            }];

                            CardLayout::new(items, Frame::new().inner_margin(16))
                                .clickable()
                                .show(ui, |ui, _idx, pointer_over| {
                                    state.hovered = pointer_over;
                                    ui.allocate_space(egui::vec2(0.0, CARD_CONTENT_HEIGHT));
                                });
                        });

                    state.visible_bottom = scroll.inner_rect.bottom();
                },
                ClippedCard::default(),
            );

        harness.run();
        harness.run();

        let below_the_scroll_area = egui::pos2(150.0, harness.state().visible_bottom + 10.0);
        harness.hover_at(below_the_scroll_area);
        harness.run();

        assert!(!harness.state().hovered);
    }
}
