use super::{AnnotationInfo, Rgba32};

impl From<u16> for AnnotationInfo {
    fn from(id: u16) -> Self {
        Self {
            id,
            label: None,
            color: None,
        }
    }
}

impl From<(u16, &str)> for AnnotationInfo {
    fn from((id, label): (u16, &str)) -> Self {
        Self {
            id,
            label: Some(label.into()),
            color: None,
        }
    }
}

impl From<(u16, &str, Rgba32)> for AnnotationInfo {
    fn from((id, label, color): (u16, &str, Rgba32)) -> Self {
        Self {
            id,
            label: Some(label.into()),
            color: Some(color),
        }
    }
}
