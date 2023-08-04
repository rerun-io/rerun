use crate::components::Color;

use super::AnnotationInfo;
use super::ClassDescription;
use super::ClassDescriptionMapElem;

impl<'s> From<(u16, &'s str)> for ClassDescriptionMapElem<'s> {
    fn from(value: (u16, &'s str)) -> Self {
        let class: ClassDescription<'s> = value.into();
        class.into()
    }
}

impl<'s> From<(u16, &'s str, Color)> for ClassDescriptionMapElem<'s> {
    fn from(value: (u16, &'s str, Color)) -> Self {
        let class: ClassDescription<'s> = value.into();
        class.into()
    }
}

impl<'s> From<AnnotationInfo<'s>> for ClassDescriptionMapElem<'s> {
    fn from(info: AnnotationInfo<'s>) -> Self {
        let class: ClassDescription<'s> = info.into();
        class.into()
    }
}

impl<'s> From<ClassDescription<'s>> for ClassDescriptionMapElem<'s> {
    fn from(class_description: ClassDescription<'s>) -> Self {
        Self {
            class_id: class_description.info.id.into(),
            class_description,
        }
    }
}
