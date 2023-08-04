use crate::components::Color;

use super::AnnotationInfo;
use super::ClassDescription;

impl<'s> From<(u16, &'s str)> for ClassDescription<'s> {
    fn from(value: (u16, &'s str)) -> Self {
        Self {
            info: value.into(),
            ..Default::default()
        }
    }
}

impl<'s> From<(u16, &'s str, Color)> for ClassDescription<'s> {
    fn from(value: (u16, &'s str, Color)) -> Self {
        Self {
            info: value.into(),
            ..Default::default()
        }
    }
}

impl<'s> From<AnnotationInfo<'s>> for ClassDescription<'s> {
    fn from(info: AnnotationInfo<'s>) -> Self {
        Self {
            info,
            ..Default::default()
        }
    }
}
