//! Support for full-span widgets.
//!
//! Full-span widgets are widgets which draw beyond the boundaries of `ui.max_rect()`, e.g. to
//! provide highlighting without margin, like [`crate::list_item`]. Typically, in the context of a
//! side panel, the full span is the entire width of the panel, excluding any margin.
//!
//! This module maintains a stack of full span values (effectively[`egui::Rangef`]) using nestable
//! scopes (via [`full_span_scope`]) and makes them available to widgets (via [`get_full_span`]).

#[derive(Clone, Default)]
struct FullSpanStack(Vec<egui::Rangef>);

/// Set up a full-span scope.
///
/// Note:
/// - Uses [`egui::Ui::scope`] internally, so it's safe to modify `ui` in the closure.
/// - Can be nested since the full-span range is stored in a stack.
pub fn full_span_scope<R>(
    ui: &mut egui::Ui,
    background_x_range: egui::Rangef,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    // push
    ui.ctx().data_mut(|writer| {
        let stack: &mut FullSpanStack = writer.get_temp_mut_or_default(egui::Id::NULL);
        stack.0.push(background_x_range);
    });

    //
    let result = ui.scope(content).inner;

    // pop
    ui.ctx().data_mut(|writer| {
        let stack: &mut FullSpanStack = writer.get_temp_mut_or_default(egui::Id::NULL);
        stack.0.pop();
    });

    result
}

/// Retrieve the current full-span scope.
///
/// If called outside a [`full_span_scope`], this function emits a warning and returns the clip
/// rectangle width. In debug build, it panics.
pub fn get_full_span(ui: &egui::Ui) -> egui::Rangef {
    let range = ui.ctx().data_mut(|writer| {
        let stack: &mut FullSpanStack = writer.get_temp_mut_or_default(egui::Id::NULL);
        stack.0.last().copied()
    });

    if range.is_none() {
        re_log::warn_once!("Full span requested outside a `full_span_scope()`");
    }
    debug_assert!(
        range.is_some(),
        "Full span requested outside a `full_span_scope()`"
    );

    range.unwrap_or(ui.clip_rect().x_range())
}
