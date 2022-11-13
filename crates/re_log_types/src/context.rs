use std::{collections::BTreeMap, sync::Arc};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationInfo {
    pub label: Option<Arc<String>>,
    pub color: Option<[u8; 4]>,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassDescription {
    pub info: AnnotationInfo,
    pub keypoint_map: BTreeMap<u16, AnnotationInfo>,
    pub skeleton_edges: BTreeMap<(u16, u16), AnnotationInfo>,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnnotationContext {
    pub class_map: BTreeMap<u16, ClassDescription>,
}
