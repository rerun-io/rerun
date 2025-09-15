use re_global_context::{DisplayMode, SystemCommand};

use crate::ViewerContext;

/// Context needed to create a shareable link.
///
/// This is also used as a utility struct to create a url from
/// the current viewer state.
pub struct UrlContext {
    pub display_mode: DisplayMode,
    pub time_range: Option<re_uri::TimeSelection>,
    pub fragment: re_uri::Fragment,
}

impl UrlContext {
    /// Create a url for a certain display mode.
    ///
    /// Not all display modes lead to valid URLs.
    pub fn new(display_mode: DisplayMode) -> Self {
        Self {
            display_mode,
            time_range: None,
            fragment: Default::default(),
        }
    }

    /// Create a url context from the current state of the viewer.
    pub fn from_context(ctx: &ViewerContext<'_>) -> Self {
        let time_ctrl = ctx.rec_cfg.time_ctrl.read();
        Self::from_context_expanded(ctx.display_mode(), Some(&time_ctrl), ctx.selection())
    }

    /// Create a url context from the current state of the viewer.
    pub fn from_context_expanded(
        display_mode: &DisplayMode,
        time_ctrl: Option<&crate::TimeControl>,
        selection: &re_global_context::ItemCollection,
    ) -> Self {
        let mut this = Self {
            display_mode: display_mode.clone(),
            time_range: None,
            fragment: re_uri::Fragment {
                selection: selection.first_item().and_then(|item| item.to_data_path()),
                when: time_ctrl.and_then(|time_ctrl| {
                    let time = time_ctrl.time_int()?;
                    Some((
                        *time_ctrl.timeline().name(),
                        re_log_types::TimeCell {
                            typ: time_ctrl.time_type(),
                            value: time.into(),
                        },
                    ))
                }),
            },
        };

        if let Some(time_ctrl) = time_ctrl {
            this = this.with_time_range(time_ctrl);
        }

        this
    }

    /// Sets the [`re_uri::Fragment`] part of this url.
    pub fn with_fragment(mut self, fragment: re_uri::Fragment) -> Self {
        self.fragment = fragment;
        self
    }

    /// Sets the timestamp this links to.
    pub fn with_timestamp(
        mut self,
        timeline: &re_chunk::Timeline,
        time: re_chunk::TimeInt,
    ) -> Self {
        self.fragment.when = Some((
            *timeline.name(),
            re_log_types::TimeCell {
                typ: timeline.typ(),
                value: time.into(),
            },
        ));

        self
    }

    /// Clears the timestamp this links to if any.
    pub fn without_timestamp(mut self) -> Self {
        self.fragment.when = None;

        self
    }

    /// Sets the trimmed time section this links to.
    pub fn with_time_range(mut self, time_ctrl: &crate::TimeControl) -> Self {
        self.time_range = time_ctrl
            .loop_selection()
            .map(|range| re_uri::TimeSelection {
                timeline: *time_ctrl.timeline(),
                range: range.to_int(),
            });
        self
    }

    /// Clears the trimmed time section this links to if any.
    pub fn without_time_range(mut self) -> Self {
        self.time_range = None;
        self
    }

    /// Creates a [`SystemCommand::CopyUrlWithContext`] to copy the link this describes.
    pub fn into_copy_cmd(self) -> SystemCommand {
        let Self {
            display_mode,
            time_range,
            fragment,
        } = self;
        SystemCommand::CopyUrlWithContext {
            display_mode,
            time_range,
            fragment,
        }
    }
}
