use std::collections::HashMap;

use egui::emath::{TSTransform, easing};

#[derive(Clone, Copy)]
pub struct EntranceAnimationItem {
    duration: f32,
    next_item_offset: f32,
    translation_y: f32,
}

impl EntranceAnimationItem {
    pub const fn new(duration: f32, next_item_offset: f32) -> Self {
        Self {
            duration,
            next_item_offset,
            translation_y: 0.0,
        }
    }

    pub const fn translated_y(mut self, translation_y: f32) -> Self {
        self.translation_y = translation_y;
        self
    }
}

#[derive(Default)]
pub struct EntranceAnimation {
    started_at: Option<f64>,
    item_started_at: HashMap<egui::Id, f64>,
    now: f64,
    cursor: f64,
    enabled: bool,
}

impl EntranceAnimation {
    pub fn begin_frame(&mut self, now: f64, enabled: bool) {
        let started_at = *self.started_at.get_or_insert(now);
        self.now = now;
        self.cursor = started_at;
        self.enabled = enabled;
    }

    pub fn add_delay(&mut self, delay: f32) {
        self.cursor += f64::from(delay);
    }

    /// `id` must identify the same logical item across frames so that inserting or removing
    /// siblings cannot move an animation that has already started.
    pub fn show<R>(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        item: EntranceAnimationItem,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let progress = self.item_progress(id, item);
        if self.enabled && progress < 1.0 {
            ui.request_repaint();
        }

        let translation_y = item.translation_y * (1.0 - progress);
        ui.with_visual_transform(
            TSTransform::from_translation(egui::vec2(0.0, translation_y)),
            |ui| {
                ui.set_opacity(progress);
                add_contents(ui)
            },
        )
    }

    fn item_progress(&mut self, id: egui::Id, item: EntranceAnimationItem) -> f32 {
        re_log::debug_assert!(item.duration > 0.0);
        re_log::debug_assert!(item.next_item_offset >= 0.0);

        let first_seen_at = self.cursor.max(self.now);
        let started_at = *self.item_started_at.entry(id).or_insert(first_seen_at);
        self.cursor = self
            .cursor
            .max(started_at + f64::from(item.next_item_offset));

        if !self.enabled {
            return 1.0;
        }

        let progress = ((self.now - started_at) as f32 / item.duration).clamp(0.0, 1.0);
        easing::cubic_out(progress)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use egui_kittest::kittest::Queryable as _;

    use super::*;

    const ITEM: EntranceAnimationItem = EntranceAnimationItem::new(0.4, 0.1);

    #[test]
    fn items_enter_in_order() {
        let mut animation = EntranceAnimation::default();
        let first_id = egui::Id::unique("first");
        let second_id = egui::Id::unique("second");
        let third_id = egui::Id::unique("third");

        animation.begin_frame(0.0, true);
        animation.item_progress(first_id, ITEM);
        animation.item_progress(second_id, ITEM);
        animation.item_progress(third_id, ITEM);

        animation.begin_frame(0.16, true);
        let first = animation.item_progress(first_id, ITEM);
        let second = animation.item_progress(second_id, ITEM);
        let third = animation.item_progress(third_id, ITEM);

        assert!(first > second);
        assert!(second > third);
    }

    #[test]
    fn inserting_an_item_does_not_move_existing_animations() {
        let mut animation = EntranceAnimation::default();
        let first = egui::Id::unique("first");
        let last = egui::Id::unique("last");

        animation.begin_frame(0.0, true);
        animation.item_progress(first, ITEM);
        animation.item_progress(last, ITEM);
        let last_started_at = animation.item_started_at[&last];

        animation.begin_frame(0.16, true);
        animation.item_progress(first, ITEM);
        let inserted_progress = animation.item_progress(egui::Id::unique("inserted"), ITEM);
        animation.item_progress(last, ITEM);

        assert_eq!(inserted_progress, 0.0);
        assert_eq!(animation.item_started_at[&last], last_started_at);
    }

    #[test]
    fn newly_visible_items_start_when_first_seen() {
        let mut animation = EntranceAnimation::default();
        let first = egui::Id::unique("first");
        let late = egui::Id::unique("late");

        animation.begin_frame(0.0, true);
        animation.item_progress(first, ITEM);

        animation.begin_frame(2.0, true);
        animation.item_progress(first, ITEM);
        assert_eq!(animation.item_progress(late, ITEM), 0.0);
        assert_eq!(animation.item_started_at[&late], 2.0);
    }

    #[test]
    fn disabled_animation_shows_everything_immediately() {
        let mut animation = EntranceAnimation::default();
        animation.begin_frame(0.0, false);

        assert_eq!(animation.item_progress(egui::Id::unique("item"), ITEM), 1.0);
    }

    #[test]
    fn animated_content_remains_interactive() {
        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_in_ui = Arc::clone(&clicked);
        let mut animation = EntranceAnimation::default();

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(200.0, 100.0))
            .build_ui(move |ui| {
                animation.begin_frame(ui.input(|input| input.time), false);
                animation.show(ui, egui::Id::unique("animated_action"), ITEM, |ui| {
                    if ui.button("Animated action").clicked() {
                        clicked_in_ui.store(true, Ordering::Relaxed);
                    }
                });
            });

        harness.get_by_label("Animated action").click();
        harness.run();

        assert!(clicked.load(Ordering::Relaxed));
    }

    #[test]
    fn animated_child_action_wins_over_parent_card() {
        let child_clicked = Arc::new(AtomicBool::new(false));
        let card_clicked = Arc::new(AtomicBool::new(false));
        let child_clicked_in_ui = Arc::clone(&child_clicked);
        let card_clicked_in_ui = Arc::clone(&card_clicked);
        let mut animation = EntranceAnimation::default();

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(200.0, 120.0))
            .build_ui(move |ui| {
                animation.begin_frame(ui.input(|input| input.time), false);

                let card_rect =
                    egui::Rect::from_min_size(ui.cursor().min, egui::vec2(180.0, 100.0));
                let card_response =
                    ui.interact(card_rect, egui::Id::unique("card"), egui::Sense::click());
                ui.advance_cursor_after_rect(card_rect);

                let mut card_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(card_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                let action_clicked = animation
                    .show(
                        &mut card_ui,
                        egui::Id::unique("card_contents"),
                        ITEM,
                        |ui| ui.button("Source").clicked(),
                    )
                    .inner;

                if action_clicked {
                    child_clicked_in_ui.store(true, Ordering::Relaxed);
                } else if card_response.clicked() {
                    card_clicked_in_ui.store(true, Ordering::Relaxed);
                }
            });

        harness.get_by_label("Source").click();
        harness.run();

        assert!(child_clicked.load(Ordering::Relaxed));
        assert!(!card_clicked.load(Ordering::Relaxed));
    }
}
