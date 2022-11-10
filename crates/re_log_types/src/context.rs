use ahash::HashMap;
use std::sync::Arc;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Info {
    pub label: Option<Arc<String>>,
    pub color: Option<[u8; 4]>,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassDescription {
    pub info: Info,
    pub keypoint_map: HashMap<u16, Info>,
    pub skeleton_edges: HashMap<(u16, u16), Info>,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationContext {
    pub class_map: HashMap<u16, ClassDescription>,
}
