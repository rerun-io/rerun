use egui::{InnerResponse, NumExt as _};

use crate::UiExt as _;
use crate::list_item::navigation::ListItemNavigation;

/// Layout statistics accumulated during the frame that are used for next frame's layout.
///
/// On frame `n`, statistics are gathered by the [`super::ListItemContent`] implementations and
/// stored in this structure (via [`LayoutInfo`] methods). Then, it is saved in egui temporary memory
/// against the scope id. On frame `n+1`, the accumulated values are used by [`list_item_scope`] to
/// set up the [`LayoutInfo`] and the accumulator is reset to restart the process.
///
/// Here is an illustration of the layout statistics that are gathered:
/// ```text
/// │◀──────────────────────get_full_span()─────────────────────▶│
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
#[derive(Debug, Clone, Default)]
struct LayoutStatistics {
    /// Maximum desired column width.
    ///
    /// The semantics are exactly the same as [`LayoutInfo`]'s `left_column_width`.
    max_desired_left_column_width: Option<f32>,

    /// Track whether any item uses the action button.
    ///
    /// If so, space for a right-aligned gutter should be reserved.
    is_action_button_used: bool,

    /// Max item width.
    ///
    /// The width is calculated from [`LayoutInfo::left_x`] to the right edge of the item.
    max_item_width: Option<f32>,

    /// `PropertyContent` only — max content width in the current scope.
    ///
    /// This value is measured from `left_x`.
    property_content_max_width: Option<f32>,
}

impl LayoutStatistics {
    /// Reset the layout statistics to the default.
    ///
    /// Should be called at the beginning of the frame.
    fn reset(ctx: &egui::Context, scope_id: egui::Id) {
        ctx.data_mut(|writer| {
            writer.insert_temp(scope_id, Self::default());
        });
    }

    /// Read the saved accumulated value.
    fn read(ctx: &egui::Context, scope_id: egui::Id) -> Self {
        if let Some(slf) = ctx.data(|reader| reader.get_temp(scope_id)) {
            slf
        } else {
            // First time we do layout in this scope.
            // The layout will likely be weird this pass,
            // so discard and do another pass to avoid jitter:
            ctx.request_discard("Missing re_ui::LayoutStatistics");
            Default::default()
        }
    }

    /// Update the accumulator.
    ///
    /// Used by [`LayoutInfo`]'s methods.
    fn update(ui: &egui::Ui, scope_id: egui::Id, update: impl FnOnce(&mut Self)) {
        ui.sanity_check();
        ui.data_mut(|writer| {
            let stats: &mut Self = writer.get_temp_mut_or_default(scope_id);
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

    /// Scope id, used to retrieve the corresponding [`LayoutStatistics`].
    scope_id: egui::Id,

    /// `PropertyContent` only — last frame's max content width, to be used in `desired_width()`
    ///
    /// This value is measured from `left_x`.
    pub(crate) property_content_max_width: Option<f32>,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        Self {
            left_x: 0.0,
            left_column_width: None,
            scope_id: egui::Id::NULL,
            property_content_max_width: None,
        }
    }
}

impl LayoutInfo {
    /// Register the desired width of the left column.
    ///
    /// All [`super::ListItemContent`] implementation that attempt to align on the two-column system should
    /// call this function once in their [`super::ListItemContent::ui`] method.
    pub fn register_desired_left_column_width(&self, ui: &egui::Ui, desired_width: f32) {
        LayoutStatistics::update(ui, self.scope_id, |stats| {
            stats.max_desired_left_column_width = stats
                .max_desired_left_column_width
                .map(|v| v.max(desired_width))
                .or(Some(desired_width));
        });
    }

    /// Indicate whether right-aligned space should be reserved for the action button.
    pub fn reserve_action_button_space(&self, ui: &egui::Ui, reserve: bool) {
        LayoutStatistics::update(ui, self.scope_id, |stats| {
            stats.is_action_button_used |= reserve;
        });
    }

    /// Register the maximum width of the item.
    ///
    /// Should only be set by [`super::ListItem`].
    pub(crate) fn register_max_item_width(&self, ui: &egui::Ui, width: f32) {
        LayoutStatistics::update(ui, self.scope_id, |stats| {
            stats.max_item_width = stats.max_item_width.map(|v| v.max(width)).or(Some(width));
        });
    }

    /// `PropertyContent` only — register max content width in the current scope
    pub(super) fn register_property_content_max_width(&self, ui: &egui::Ui, width: f32) {
        LayoutStatistics::update(ui, self.scope_id, |stats| {
            stats.property_content_max_width = stats
                .property_content_max_width
                .map(|v| v.max(width))
                .or(Some(width));
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
            let stack: &mut Self = writer.get_temp_mut_or_default(egui::Id::NULL);
            stack.0.push(state);
        });
    }

    fn pop(ctx: &egui::Context) -> Option<LayoutInfo> {
        ctx.data_mut(|writer| {
            let stack: &mut Self = writer.get_temp_mut_or_default(egui::Id::NULL);
            stack.0.pop()
        })
    }

    /// Returns the current [`LayoutInfo`] to be used by [`super::ListItemContent`] implementation.
    ///
    /// # Panics
    ///
    /// This function panics if the stack is temps. [`super::ListItem`] must always be nested in a
    /// [`list_item_scope`].
    pub(crate) fn top(ctx: &egui::Context) -> LayoutInfo {
        ctx.data_mut(|writer| {
            let stack: &mut Self = writer.get_temp_mut_or_default(egui::Id::NULL);
            let state = stack.0.last();
            if state.is_none() {
                re_log::warn_once!(
                    "Attempted to access empty LayoutInfo stack, returning default LayoutInfo. \
                    Wrap all calls to ListItem in a list_item_scope()."
                );
            }
            re_log::debug_assert!(
                state.is_some(),
                "ListItem was not wrapped in list_item_scope()"
            );
            state.cloned().unwrap_or_default()
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
/// *Note*
/// - The scope id is derived from the provided `id_salt` and combined with the [`egui::Ui`]'s id,
///   such that `id_salt` only needs to be unique within the scope of the parent ui.
/// - Creates a new wrapped [`egui::Ui`] internally, so it's safe to modify the `ui` within the closure.
/// - Uses [`egui::Ui::push_id`] so two sibling `list_item_scope`:s with different ids won't have id clashes within them.
/// - The `ui.spacing_mut().item_spacing.y` is set to `0.0` to remove the default spacing between
///   list items.
pub fn list_item_scope<R>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> InnerResponse<R> {
    ui.sanity_check();

    let id_salt = egui::Id::new(id_salt); // So we can use it twice
    let scope_id = ui.id().with(id_salt);

    // read last frame layout statistics and reset for the new frame
    let layout_stats = LayoutStatistics::read(ui.ctx(), scope_id);
    LayoutStatistics::reset(ui.ctx(), scope_id);

    // prepare the layout infos
    let left_column_width =
        if let Some(max_desired_left_column_width) = layout_stats.max_desired_left_column_width {
            // TODO(ab): this heuristics can certainly be improved, to be done with more hindsight
            // from real-world usage.
            let available_width = layout_stats
                .max_item_width
                .unwrap_or_else(|| ui.available_width());
            Some(max_desired_left_column_width.at_most(0.7 * available_width))
        } else {
            None
        };
    let state = LayoutInfo {
        left_x: ui.max_rect().left(),
        left_column_width,
        scope_id,
        property_content_max_width: layout_stats.property_content_max_width,
    };

    let is_root = ListItemNavigation::init_if_root(ui.ctx());

    // push, run, pop
    LayoutInfoStack::push(ui.ctx(), state.clone());
    let response = ui.push_id(id_salt, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        content(ui)
    });
    LayoutInfoStack::pop(ui.ctx());

    if is_root {
        ListItemNavigation::end_if_root(ui.ctx());
    }

    ui.sanity_check();

    response
}
