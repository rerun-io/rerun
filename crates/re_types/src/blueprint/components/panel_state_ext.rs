use super::PanelState;

impl PanelState {
    /// Returns `true` if self is [`PanelState::Expanded`]
    #[inline]
    pub fn is_expanded(&self) -> bool {
        self == &Self::Expanded
    }

    /// Returns `true` if self is [`PanelState::Hidden`]
    #[inline]
    pub fn is_hidden(&self) -> bool {
        self == &Self::Hidden
    }

    /// Sets the panel to [`Self::Hidden`] if it is collapsed or expanded, and [`Self::Expanded`] if it is hidden.
    #[inline]
    pub fn toggle(self) -> Self {
        match self {
            Self::Collapsed | Self::Hidden => Self::Expanded,
            Self::Expanded => Self::Collapsed,
        }
    }
}
