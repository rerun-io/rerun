/// Describes the class of a dataset layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum LayerClass {
    /// Asset layers use a single recording for all the segments in the layer.
    ///
    /// This is used to deduplicate data, so that e.g. a dataset of a robot arm
    /// only needs to store the URDF for that robot arm once.
    Asset,

    /// Segment layers have one (or zero) recordings per segment in the layer.
    ///
    /// This layer type is used when every recording in the layer is different.
    Segment,
}
