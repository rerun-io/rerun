use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

/// Specifies that the entity path at which this is logged is disconnected from its parent.
///
/// If a transform or pinhole is logged on the same path, this component will be ignored.
///
/// This is useful for specifying that a subgraph is independent of the rest of the scene.
///
/// This component is a "mono-component". See [the crate level docs](crate) for details.
#[derive(Copy, Clone, Debug, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[arrow_field(transparent)]
#[repr(transparent)]
pub struct DisconnectedSpace(bool);

impl DisconnectedSpace {
    #[inline]
    pub fn new() -> Self {
        Self(false)
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
