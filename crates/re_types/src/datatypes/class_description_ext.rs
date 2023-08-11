use super::{AnnotationInfo, ClassDescription, Color};

impl From<(u16, &str)> for ClassDescription {
    fn from(value: (u16, &str)) -> Self {
        Self {
            info: value.into(),
            ..Default::default()
        }
    }
}

impl From<(u16, &str, Color)> for ClassDescription {
    fn from(value: (u16, &str, Color)) -> Self {
        Self {
            info: value.into(),
            ..Default::default()
        }
    }
}

impl From<AnnotationInfo> for ClassDescription {
    fn from(info: AnnotationInfo) -> Self {
        Self {
            info,
            ..Default::default()
        }
    }
}
