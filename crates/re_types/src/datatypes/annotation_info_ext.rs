use super::{AnnotationInfo, Color};

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

impl From<(u16, &str, Color)> for AnnotationInfo {
    fn from((id, label, color): (u16, &str, Color)) -> Self {
        Self {
            id,
            label: Some(label.into()),
            color: Some(color),
        }
    }
}
