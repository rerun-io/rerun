use crate::components::Color;

use super::AnnotationInfo;

impl<'s> From<(u16, &'s str)> for AnnotationInfo<'s> {
    fn from(value: (u16, &'s str)) -> Self {
        Self {
            id: value.0,
            label: Some(value.1.into()),
            color: None,
        }
    }
}

impl<'s> From<(u16, &'s str, Color)> for AnnotationInfo<'s> {
    fn from(value: (u16, &'s str, Color)) -> Self {
        Self {
            id: value.0,
            label: Some(value.1.into()),
            color: Some(value.2),
        }
    }
}
