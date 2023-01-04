use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

#[derive(Debug, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct Size3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Component for Size3D {
    fn name() -> crate::ComponentName {
        "rerun.size3d".into()
    }
}
