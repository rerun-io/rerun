use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

// TODO:
#[derive(Copy, Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DisconnectedSpace {
    placeholder: bool,
}

impl DisconnectedSpace {
    #[inline]
    pub fn new() -> Self {
        Self { placeholder: false }
    }
}

impl Default for DisconnectedSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DisconnectedSpace {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.disconnected_space".into()
    }
}
