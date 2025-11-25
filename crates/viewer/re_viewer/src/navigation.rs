use re_viewer_context::DisplayMode;
use vec1::smallvec_v1::SmallVec1;

/// The navigation history of the viewer.
///
/// This object should never be exposed to directly via contexts. Instead,
/// we retrieve the display mode and pass that around.
pub(crate) struct Navigation {
    stack: SmallVec1<[DisplayMode; 2]>,
    start_mode: DisplayMode,
}

impl Default for Navigation {
    fn default() -> Self {
        let start_mode = DisplayMode::RedapServer(re_redap_browser::EXAMPLES_ORIGIN.clone());
        Self {
            stack: SmallVec1::new(start_mode.clone()),
            start_mode,
        }
    }
}

impl Navigation {
    // TODO(grtlr): In the future we should have something like `push_unique`, but for
    // this we first need all display modes to contain more information.

    pub fn push(&mut self, display_mode: DisplayMode) {
        re_log::debug!("Pushed display mode `{:?}`", display_mode);
        self.stack.push(display_mode);
    }

    /// Resets to use the start display mode, which is also the fallback mode for
    /// navigation.
    ///
    /// This is defined in the default implementation for [`Navigation`]
    pub fn reset(&mut self) {
        if *self.current() != self.start_mode {
            if self.stack.len() > 1 {
                self.stack.drain(1..).expect("We checked length");
            }
            *self.stack.last_mut() = self.start_mode.clone();
        }
    }

    pub fn replace(&mut self, new_mode: DisplayMode) -> DisplayMode {
        let previous = std::mem::replace(self.stack.last_mut(), new_mode);

        if previous != *self.current() {
            re_log::trace!("Navigated from {previous:?} to {:?}", self.current());
        }

        previous
    }

    pub fn pop(&mut self) -> Option<DisplayMode> {
        let previous = self.stack.pop().ok();
        re_log::debug!("Popped display mode `{:?}`", previous);
        previous
    }

    /// Current state
    pub fn current(&self) -> &DisplayMode {
        self.stack.last()
    }
}
