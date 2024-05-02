use egui::NumExt;
use once_cell::sync::Lazy;

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// X coordinate span to use for hover/selection highlight.
    ///
    /// Note: this field is not, strictly speaking, part of the state, as it's overwritten with each
    /// call of `list_item_scope`. Still, it's convenient to have it here to pass it from the scope
    /// to the inner `ListItem`.
    // TODO(#6156): this being here is a (temporary) hack (see docstring). In the future, this will
    // be generalized to some `full_span_scope` mechanism to be used by all full-span widgets beyond
    // `ListItem`.
    pub(crate) background_x_range: egui::Rangef,

    /// Left-most X coordinate for the scope.
    ///
    /// This is the reference point for tracking column width. This is set by [`list_item_scope`]
    /// based on `ui.max_rect()`.
    pub(crate) left_x: f32,

    /// Column width to be used this frame.
    ///
    /// The column width has `left_x` as reference, so it includes:
    /// - All the indentation on the left side of the list item.
    /// - Any extra indentation added by the list item itself.
    /// - The list item's collapsing triangle, if any.
    ///
    /// The effective left column width for a given [`super::ListItemContent`] implementation can be
    /// calculated as `left_column_width - (context.rect.left() - left_x)`.
    pub(crate) left_column_width: Option<f32>,

    /// Maximum desired column width, to be updated this frame.
    ///
    /// The semantics are exactly the same as for `left_column_width`.
    max_desired_left_column_width: f32,
    /**/
    // TODO(#6179): record the use of right action button in all PropertyContent such as to not
    // unnecessarily reserve right gutter space if none have it.
}

impl Default for State {
    fn default() -> Self {
        Self {
            background_x_range: egui::Rangef::NOTHING,
            left_x: f32::NEG_INFINITY,
            left_column_width: None,
            max_desired_left_column_width: f32::NEG_INFINITY,
        }
    }
}

impl State {
    /// Register the desired width of the left column.
    ///
    /// All [`super::ListItemContent`] implementation that attempt to align on the two-column system should
    /// call this function once in their [`super::ListItemContent::ui`] method.
    pub(crate) fn register_desired_left_column_width(&mut self, desired_width: f32) {
        self.max_desired_left_column_width = self.max_desired_left_column_width.max(desired_width);
    }
}

/// Stack of [`State`]s.
///
/// The stack is stored in `egui`'s memory and its API directly wraps the relevant calls.
/// Calls to [`list_item_scope`] push new states to the stack so that [`super::ListItem`]s can
/// always access the correct state from the top of the stack.
#[derive(Debug, Clone, Default)]
pub(crate) struct StateStack(Vec<State>);

static STATE_STACK_ID: Lazy<egui::Id> = Lazy::new(|| egui::Id::new("re_ui_list_item_state_stack"));

impl StateStack {
    fn push(ctx: &egui::Context, state: State) {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            stack.0.push(state);
        });
    }

    fn pop(ctx: &egui::Context) -> Option<State> {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            stack.0.pop()
        })
    }

    /// Returns the current [`State`] to be used by [`super::ListItemContent`] implementation.
    ///
    /// For ergonomic reasons, this function will fail by returning a default state if the stack is
    /// empty. This is an error condition that should be addressed by wrapping `ListItem` code in a
    /// [`super::list_item_scope`].
    pub(crate) fn top(ctx: &egui::Context) -> State {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            let state = stack.0.last();
            if state.is_none() {
                re_log::warn_once!(
                    "Attempted to access empty ListItem state stack, returning default state. \
                    Wrap in a `list_item_scope`."
                );
            }
            state.cloned().unwrap_or_default()
        })
    }

    /// Provides mutable access to the current [`State`].
    ///
    /// The closure is called with a mutable reference to the current state, if any. If the stack is
    /// empty, the closure is not called and a warning is logged.
    pub(crate) fn top_mut(ctx: &egui::Context, state_writer: impl FnOnce(&mut State)) {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            let state = stack.0.last_mut();
            if let Some(state) = state {
                state_writer(state);
            } else {
                re_log::warn_once!(
                    "Failed to mutable access empty ListItem state stack. Wrap in a \
                    `list_item_scope`."
                );
            }
        });
    }

    fn peek(ctx: &egui::Context) -> Option<State> {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            stack.0.last().cloned()
        })
    }
}

/// Create a scope in which `[ListItem]`s can be created.
///
/// This scope serves two purposes:
/// - Manage the state that is saved across frame (e.g. for tracking column boundary position).
/// - Manage the range of X coordinates defining the boundaries of the hover/selection highlight.
///
/// State is loaded against the provided `id`, and pushed to a global stack, such that calls to this
/// function may be nested. `ListItem` code will always use the top of the stack as current state.
///
/// The hover/selection highlight coordinate range is determined with the following heuristics:
/// - Value passed as argument if not `None`.
/// - Value from the parent scope if the scope is nested.
/// - Clip rectangle's `x_range` (legacy behavior).
///
/// Given the above, `list_item_scope` can be used for two potentially distinct use-cases:
/// 1) Store a suitable `background_x_range` value for use by nested `ListItem`s. This happens,
///    e.g., close to the top of the `egui::SidePanel::show` closure, where the panel size
///    information is readily available (e.g. `ui.max_rect().x_range()`).
/// 2) Limit state sharing for a subgroup of `ListItem`s. This makes it possible to independently
///    align the columns of two `ListItem`s subgroups, for which a single, global alignment would
///    be detrimental. This may happen in deeply nested UI code.
///
pub fn list_item_scope<R>(
    ui: &mut egui::Ui,
    id: impl Into<egui::Id>,
    background_x_range: Option<egui::Rangef>,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    /*
    data contains two set of things:
    - some per container state
    - a global state stack that is read by actual list items
     */

    let id = id.into();

    // read the state for this container, if any
    let state: Option<State> = ui.data(|reader| reader.get_temp(id));
    let mut state = state.unwrap_or_default();

    // determine the background x range to use
    // TODO(#6156): the background X range stuff is to be split off and generalised for all full-span
    // widgets.
    state.background_x_range = if let Some(background_x_range) = background_x_range {
        background_x_range
    } else if let Some(parent_state) = StateStack::peek(ui.ctx()) {
        parent_state.background_x_range
    } else {
        ui.clip_rect().x_range()
    };

    // Set up the state for this scope.
    state.left_x = ui.max_rect().left();
    state.left_column_width = if state.max_desired_left_column_width > 0.0 {
        Some(
            // TODO(ab): this heuristics can certainly be improved, to be done with more hindsight
            // from real-world usage.
            state
                .max_desired_left_column_width
                .at_most(0.7 * ui.max_rect().width()),
        )
    } else {
        None
    };
    state.max_desired_left_column_width = f32::NEG_INFINITY;

    // push, run, pop
    StateStack::push(ui.ctx(), state.clone());
    let result = content(ui);
    let state = StateStack::pop(ui.ctx());

    // save the state for this container
    if let Some(state) = state {
        ui.data_mut(|writer| writer.insert_temp(id, state));
    }

    result
}
