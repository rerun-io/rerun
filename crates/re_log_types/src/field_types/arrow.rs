#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Arrow3D {
    pub origin: [f32; 3],
    pub vector: [f32; 3],
}
