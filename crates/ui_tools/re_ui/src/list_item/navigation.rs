use egui::collapsing_header::CollapsingState;
use egui::{Context, Id, Modifiers};

#[derive(Debug, Clone, Default)]
pub struct ListItemNavigation {
    /// Whenever entering a collapsible list item, push its id here.
    pub parent_stack: Vec<Id>,

    /// This should be set by every list item _until_ `Self::current_focused` is Some.
    pub previous_item: Option<Id>,

    /// This should be set by the focused list item.
    pub current_focused: Option<Id>,

    /// Is the focused item collapsible? What is its current state?
    ///
    /// None if the focused item is not collapsible.
    pub focused_collapsed: Option<bool>,

    /// What's the parent of the focused item?
    ///
    /// This should be the last item in `parent_stack` when `current_focused` is set.
    pub focused_parent: Option<Id>,

    /// This should be set if it is [`None`] and `Self::current_focused` is Some.
    pub next_item: Option<Id>,
}

impl ListItemNavigation {
    /// Returns true if this is the root scope.
    ///
    /// If it is, you should call [`Self::end_if_root`] after showing the contents.
    pub fn init_if_root(ctx: &Context) -> bool {
        ctx.data_mut(|d| {
            let root = d.get_temp::<Self>(Id::NULL).is_none();

            if root {
                d.get_temp_mut_or_default::<Self>(Id::NULL);
            }

            root
        })
    }

    pub fn with_mut(ctx: &Context, f: impl FnOnce(&mut Self)) {
        ctx.data_mut(|d| {
            // Having a `has_temp` and/or `get_temp_mut` would be nice…
            if d.get_temp::<Self>(Id::NULL).is_some() {
                f(d.get_temp_mut_or_insert_with(Id::NULL, || unreachable!()));
            }
        });
    }

    /// Did we gain focus via arrow key navigation last pass?
    ///
    /// (As opposed to e.g. mouse click or tab key)
    pub fn gained_focus_via_arrow_key(ctx: &Context, list_item_id: Id) -> bool {
        let pass = ctx.cumulative_pass_nr();
        ctx.memory_mut(|mem| {
            if mem.focused() == Some(list_item_id) {
                let gained_via_arrow = mem.data.get_temp(Id::NULL);
                gained_via_arrow.is_some_and(
                    |GainedFocusViaArrowKey {
                         widget_id: id,
                         focused_on_pass,
                     }| { id == list_item_id && focused_on_pass == pass - 1 },
                )
            } else {
                false
            }
        })
    }

    /// Call this _only_ if [`Self::init_if_root`] returned true.
    pub fn end_if_root(ctx: &Context) {
        let navigation = ctx.data_mut(|d| d.remove_temp::<Self>(Id::NULL));
        re_log::debug_assert!(navigation.is_some(), "Expected to find ListItemNavigation");

        let Some(navigation) = navigation else {
            return;
        };
        let Some(current_focused) = navigation.current_focused else {
            return;
        };

        let mut focus_item = None;
        let mut collapse_item = None;
        let mut expand_item = None;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, egui::Key::ArrowUp) {
                match (navigation.focused_collapsed, navigation.focused_parent) {
                    // The current item is expanded, so collapse it.
                    (Some(false), _) => {
                        collapse_item = Some(current_focused);
                    }
                    // The current item is collapsed or not collapsible, so focus its parent.
                    (Some(true) | None, Some(parent)) => {
                        focus_item = Some(parent);
                    }
                    // Focused item is collapsed and has no parent, do nothing.
                    (Some(true) | None, None) => {}
                }
            }

            if i.consume_key(Modifiers::COMMAND, egui::Key::ArrowDown) {
                if navigation.focused_collapsed == Some(true) {
                    expand_item = Some(current_focused);
                } else {
                    // If it's expanded or not collapsible, focus the next item.
                    focus_item = navigation.next_item;
                }
            }

            if let Some(previous) = navigation.previous_item
                && i.consume_key(Modifiers::NONE, egui::Key::ArrowUp)
            {
                focus_item = Some(previous);
            }

            if let Some(next) = navigation.next_item
                && i.consume_key(Modifiers::NONE, egui::Key::ArrowDown)
            {
                focus_item = Some(next);
            }
        });

        if let Some(next_focus) = focus_item {
            let pass = ctx.cumulative_pass_nr();
            ctx.memory_mut(|mem| mem.request_focus(next_focus));
            ctx.data_mut(|d| {
                d.insert_temp(
                    Id::NULL,
                    GainedFocusViaArrowKey {
                        widget_id: next_focus,
                        focused_on_pass: pass,
                    },
                );
            });
            if let Some(response) = ctx.read_response(next_focus) {
                response.scroll_to_me(None);
            }
        }
        if let Some(collapse_item) = collapse_item {
            if let Some(mut state) = CollapsingState::load(ctx, collapse_item) {
                state.set_open(false);
                state.store(ctx);
            }
        } else if let Some(expand_item) = expand_item
            && let Some(mut state) = CollapsingState::load(ctx, expand_item)
        {
            state.set_open(true);
            state.store(ctx);
        }
    }
}

/// Utility to check if focus was gained via arrow key navigation.
///
/// We need this because some UI parts (e.g. blueprint tree) will want to auto-select focused item
/// _only_ if they were focused with arrows, not if they were focused e.g. with tab.
#[derive(Debug, Clone)]
struct GainedFocusViaArrowKey {
    widget_id: Id,
    focused_on_pass: u64,
}
