use crate::datatypes::ClassDescriptionMapElem;

use super::AnnotationContext;

impl AnnotationContext {
    pub fn new<T: Into<ClassDescriptionMapElem>, const N: usize>(value: [T; N]) -> Self {
        Self(value.into_iter().map(|v| v.into()).collect())
    }
}
