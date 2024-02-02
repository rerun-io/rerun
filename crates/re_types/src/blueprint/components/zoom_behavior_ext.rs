use super::ZoomBehavior;

// TODO(#3384): Code-gen all of this
// Pseudo-enum
#[allow(non_upper_case_globals)]
impl ZoomBehavior {
    pub const PreserveAspectRatio: ZoomBehavior = ZoomBehavior(1);
    pub const LockToRange: ZoomBehavior = ZoomBehavior(2);
}

impl Default for ZoomBehavior {
    fn default() -> Self {
        Self::PreserveAspectRatio
    }
}

impl std::fmt::Display for ZoomBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            1 => write!(f, "Preserve Aspect Ratio"),
            2 => write!(f, "Lock to Range"),
            _ => write!(f, "Unknown zoom behavior value: {}", self.0),
        }
    }
}
