use arrow2_convert::ArrowField;

use crate::msg_bundle::Component;

#[derive(Debug, ArrowField)]
pub struct Size3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Component for Size3D {
    const NAME: crate::ComponentNameRef<'static> = "rerun.size3d";
}
