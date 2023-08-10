use super::{AnnotationInfo, Color};

impl From<(u16, &str)> for AnnotationInfo {
    fn from(value: (u16, &str)) -> Self {
        Self {
            id: value.0,
            label: Some(value.1.into()),
            color: None,
        }
    }
}

impl From<(u16, &str, Color)> for AnnotationInfo {
    fn from(value: (u16, &str, Color)) -> Self {
        Self {
            id: value.0,
            label: Some(value.1.into()),
            color: Some(value.2),
        }
    }
}
