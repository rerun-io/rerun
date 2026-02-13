use super::{AnnotationInfo, ClassDescription, Rgba32};

impl From<(u16, &str)> for ClassDescription {
    fn from(value: (u16, &str)) -> Self {
        Self {
            info: value.into(),
            ..Default::default()
        }
    }
}

impl From<(u16, &str, Rgba32)> for ClassDescription {
    fn from(value: (u16, &str, Rgba32)) -> Self {
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
