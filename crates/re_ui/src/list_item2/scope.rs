use egui::NumExt;

/// Layout statistics accumulated during the frame that are used for next frame's layout.
///
/// On frame `n`, statistics are gathered by the [`super::ListItemContent`] implementations and
/// stored in this structure (via [`LayoutInfo`] methods). Then, it is saved in egui temporary memory
/// against the scope id. On frame `n+1`, the accumulated values are used by [`list_item_scope`] to
/// set up the [`LayoutInfo`] and the accumulator is reset to restart the process.
///
/// Here is an illustration of the layout statistics that are gathered:
/// ```text
/// │◀─────────────────────background_x_range───────────────────▶│
/// │                                                            │
/// │  ┌──left_x                                                 │
/// │  ▼                                                         │
/// │  │                       │                        │        │
/// │  ┌───────────────────────────────────────────┐             │
/// │  │                       │                   │    │        │
/// │  └───┬────────────────────────────────────┬──┘             │
/// │  │ ▼ │                   │                │       │        │
/// │      └───┬─────────────────────────┬──────┘                │
/// │  │       │               │         │              │        │
/// │          ├─────────────────────────┴────┐                  │
/// │  │     ▼ │               │              │         │        │
/// │          └───┬──────────────────────────┴─────────┐        │
/// │  │           │           │                        │        │
/// │              ├─────────────────────┬──────────────┘        │
/// │  │         ▶ │           │         │              │        │
/// │  ┌───────────┴─────────────────────┴──┐                    │
/// │  │                       │            │           │        │
/// │  └────────────────────────────────────┘                    │
/// │  │                       │                        │        │
/// │                                                            │
/// │  │◀──────────────────────▶ max_desired_left_column_width   │
/// │                                                            │
/// │  │◀───────────────max_item_width─────────────────▶│        │
/// ```
#[derive(Debug, Clone)]
struct LayoutStatistics {
    /// Maximum desired column width.
    ///
    /// The semantics are exactly the same as [`LayoutInfo`]'s `left_column_width`.
    max_desired_left_column_width: f32,

    /// Track whether any item uses the action button.
    ///
    /// If so, space for a right-aligned gutter should be reserved.
    is_action_button_used: bool,

    /// Max item width.
    ///
    /// The width is calculated from [`LayoutInfo::left_x`] to the right edge of the item.
    max_item_width: f32,
}

impl Default for LayoutStatistics {
    fn default() -> Self {
        // set values suitable to initialize the stat accumulator
        Self {
            max_desired_left_column_width: f32::NEG_INFINITY,
            is_action_button_used: false,
            max_item_width: f32::NEG_INFINITY,
        }
    }
}

impl LayoutStatistics {
    /// Reset the layout statistics to the default.
    ///
    /// Should be called at the beginning of the frame.
    fn reset(ctx: &egui::Context, scope_id: egui::Id) {
        ctx.data_mut(|writer| {
            writer.insert_temp(scope_id, LayoutStatistics::default());
        });
    }

    /// Read the saved accumulated value.
    fn read(ctx: &egui::Context, scope_id: egui::Id) -> LayoutStatistics {
        ctx.data(|reader| reader.get_temp(scope_id).unwrap_or_default())
    }

    /// Update the accumulator.
    ///
    /// Used by [`LayoutInfo`]'s methods.
    fn update(ctx: &egui::Context, scope_id: egui::Id, update: impl FnOnce(&mut LayoutStatistics)) {
        ctx.data_mut(|writer| {
            let stats: &mut LayoutStatistics = writer.get_temp_mut_or_default(scope_id);
            update(stats);
        });
    }
}

/// Layout information prepared by [`list_item_scope`] to be used by [`super::ListItemContent`].
///
/// This structure has two purposes:
/// - Provide read-only layout information to be used when rendering the list item.
/// - Provide an API to register needs (such as left column width). These needs are then accumulated
///   and used to set up the next frame's layout information.
///
/// [`super::ListItemContent`] implementations have access to this structure via
/// [`super::ContentContext`].
#[derive(Debug, Clone)]
pub struct LayoutInfo {
    /// X coordinate span to use for hover/selection highlight.
    // TODO(#6156): this being here is a (temporary) hack (see docstring). In the future, this will
    // be generalized to some `full_span_scope` mechanism to be used by all full-span widgets beyond
    // `ListItem`.
    pub(crate) background_x_range: egui::Rangef,

    /// Left-most X coordinate for the scope.
    ///
    /// This is the reference point for tracking column width. This is set by [`list_item_scope`]
    /// based on `ui.max_rect()`.
    pub(crate) left_x: f32,

    /// Column width to be read this frame.
    ///
    /// The column width has `left_x` as reference, so it includes:
    /// - All the indentation on the left side of the list item.
    /// - Any extra indentation added by the list item itself.
    /// - The list item's collapsing triangle, if any.
    ///
    /// The effective left column width for a given [`super::ListItemContent`] implementation can be
    /// calculated as `left_column_width - (context.rect.left() - left_x)`.
    ///
    /// This value is set to `None` during the first frame, when [`list_item_scope`] isn't able to
    /// determine a suitable value. In that case, implementations should devise a suitable default
    /// value.
    pub(crate) left_column_width: Option<f32>,

    /// If true, right-aligned space should be reserved for the action button, even if not used.
    pub(crate) reserve_action_button_space: bool,

    /// Scope id, used to retrieve the corresponding [`LayoutStatistics`].
    scope_id: egui::Id,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        Self {
            background_x_range: egui::Rangef::NOTHING,
            left_x: f32::NEG_INFINITY,
            left_column_width: None,
            reserve_action_button_space: true,
            scope_id: egui::Id::NULL,
        }
    }
}

impl LayoutInfo {
    /// Register the desired width of the left column.
    ///
    /// All [`super::ListItemContent`] implementation that attempt to align on the two-column system should
    /// call this function once in their [`super::ListItemContent::ui`] method.
    pub fn register_desired_left_column_width(&self, ctx: &egui::Context, desired_width: f32) {
        LayoutStatistics::update(ctx, self.scope_id, |stats| {
            stats.max_desired_left_column_width =
                stats.max_desired_left_column_width.max(desired_width);
        });
    }

    /// Indicate whether right-aligned space should be reserved for the action button.
    pub fn reserve_action_button_space(&self, ctx: &egui::Context, reserve: bool) {
        LayoutStatistics::update(ctx, self.scope_id, |stats| {
            stats.is_action_button_used |= reserve;
        });
    }

    /// Register the maximum width of the item.
    ///
    /// Should only be set by [`super::ListItem`].
    pub(crate) fn register_max_item_width(&self, ctx: &egui::Context, width: f32) {
        LayoutStatistics::update(ctx, self.scope_id, |stats| {
            stats.max_item_width = stats.max_item_width.max(width);
        });
    }
}

/// Stack of [`LayoutInfo`]s.
///
/// The stack is stored in `egui`'s memory and its API directly wraps the relevant calls.
/// Calls to [`list_item_scope`] push new [`LayoutInfo`] to the stack so that [`super::ListItem`]s
/// can always access the correct state from the top of the stack.
///
/// [`super::ListItemContent`] implementations should *not* access the stack directly but instead
/// use the [`LayoutInfo`] provided by [`super::ContentContext`].
#[derive(Debug, Clone, Default)]
pub(crate) struct LayoutInfoStack(Vec<LayoutInfo>);

impl LayoutInfoStack {
    fn push(ctx: &egui::Context, state: LayoutInfo) {
        ctx.data_mut(|writer| {
            let stack: &mut LayoutInfoStack = writer.get_temp_mut_or_default(egui::Id::NULL);
            stack.0.push(state);
        });
    }

    fn pop(ctx: &egui::Context) -> Option<LayoutInfo> {
        ctx.data_mut(|writer| {
            let stack: &mut LayoutInfoStack = writer.get_temp_mut_or_default(egui::Id::NULL);
            stack.0.pop()
        })
    }

    /// Returns the current [`LayoutInfo`] to be used by [`super::ListItemContent`] implementation.
    ///
    /// For ergonomic reasons, this function will fail by returning a default state if the stack is
    /// empty. This is an error condition that should be addressed by wrapping `ListItem` code in a
    /// [`super::list_item_scope`].
    pub(crate) fn top(ctx: &egui::Context) -> LayoutInfo {
        ctx.data_mut(|writer| {
            let stack: &mut LayoutInfoStack = writer.get_temp_mut_or_default(egui::Id::NULL);
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

    fn peek(ctx: &egui::Context) -> Option<LayoutInfo> {
        ctx.data_mut(|writer| {
            let stack: &mut LayoutInfoStack = writer.get_temp_mut_or_default(egui::Id::NULL);
            stack.0.last().cloned()
        })
    }
}

/// Create a scope in which `[ListItem]`s can be created.
///
/// This scope provides the infrastructure to gather layout statistics from nested list items,
/// compute corresponding layout information, and provide this information to nested list items.
///
/// State is loaded against the scope id, and pushed to a global stack, such that calls to this
/// function may be nested. `ListItem` code will always use the top of the stack as current state.
///
/// Layout statistics are accumulated during the frame and stored in egui's memory against the scope
/// id. Layout information is pushed to a global stack, which is also stored in egui's memory. This
/// enables nesting [`list_item_scope`]s.
///
/// *Note*: the scope id is derived from the provided `id_source` and combined with the
/// [`egui::Ui`]'s id, such that `id_source` only needs to be unique within the scope of the parent
/// ui.
pub fn list_item_scope<R>(
    ui: &mut egui::Ui,
    id_source: impl Into<egui::Id>,
    background_x_range: Option<egui::Rangef>,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let scope_id = ui.id().with(id_source.into());

    // read last frame layout statistics and reset for the new frame
    let layout_stats = LayoutStatistics::read(ui.ctx(), scope_id);
    LayoutStatistics::reset(ui.ctx(), scope_id);

    // prepare the layout infos
    // TODO(#6156): the background X range stuff is to be split off and generalised for all full-span
    // widgets.
    let background_x_range = if let Some(background_x_range) = background_x_range {
        background_x_range
    } else if let Some(parent_state) = LayoutInfoStack::peek(ui.ctx()) {
        parent_state.background_x_range
    } else {
        ui.clip_rect().x_range()
    };
    let left_column_width = if layout_stats.max_desired_left_column_width > 0.0 {
        Some(
            // TODO(ab): this heuristics can certainly be improved, to be done with more hindsight
            // from real-world usage.
            layout_stats
                .max_desired_left_column_width
                .at_most(0.7 * layout_stats.max_item_width),
        )
    } else {
        None
    };
    let state = LayoutInfo {
        background_x_range,
        left_x: ui.max_rect().left(),
        left_column_width,
        reserve_action_button_space: layout_stats.is_action_button_used,
        scope_id,
    };

    // push, run, pop
    LayoutInfoStack::push(ui.ctx(), state);
    let result = content(ui);
    LayoutInfoStack::pop(ui.ctx());

    result
}
