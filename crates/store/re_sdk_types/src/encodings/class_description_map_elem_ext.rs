use super::{AnnotationInfo, ClassDescription, ClassDescriptionMapElem, Rgba32};

impl From<(u16, &str)> for ClassDescriptionMapElem {
    fn from(value: (u16, &str)) -> Self {
        let class: ClassDescription = value.into();
        class.into()
    }
}

impl From<(u16, &str, Rgba32)> for ClassDescriptionMapElem {
    fn from(value: (u16, &str, Rgba32)) -> Self {
        let class: ClassDescription = value.into();
        class.into()
    }
}

impl From<AnnotationInfo> for ClassDescriptionMapElem {
    fn from(info: AnnotationInfo) -> Self {
        let class: ClassDescription = info.into();
        class.into()
    }
}

impl From<ClassDescription> for ClassDescriptionMapElem {
    fn from(class_description: ClassDescription) -> Self {
        Self {
            class_id: class_description.info.id.into(),
            class_description,
        }
    }
}
