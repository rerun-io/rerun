use crate::{
    path::{FieldName, ObjPath},
    ComponentName,
};

/// A `DataPath` may contain either a classic `Field` or an arrow `Component`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum FieldOrComponent {
    Field(FieldName),
    Component(ComponentName),
}

impl FieldOrComponent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Field(field) | Self::Component(field) => field.as_str(),
        }
    }
}

impl std::fmt::Display for FieldOrComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let field = match self {
            Self::Field(field) | Self::Component(field) => field,
        };
        field.fmt(f)
    }
}

/// A [`ObjPath`] plus a [`FieldName`].
///
/// Example: `camera / "left" / points / #42`.`pos`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataPath {
    /// `camera / "left" / points / #42`
    pub obj_path: ObjPath,

    /// "pos"
    pub field_name: FieldOrComponent,
}

impl DataPath {
    #[inline]
    pub fn new(obj_path: ObjPath, field_name: FieldName) -> Self {
        Self {
            obj_path,
            field_name: FieldOrComponent::Field(field_name),
        }
    }

    #[inline]
    pub fn new_arrow(obj_path: ObjPath, component_name: ComponentName) -> Self {
        Self {
            obj_path,
            field_name: FieldOrComponent::Component(component_name),
        }
    }

    #[inline]
    pub fn new_any(obj_path: ObjPath, field_name: FieldOrComponent) -> Self {
        Self {
            obj_path,
            field_name,
        }
    }

    #[inline]
    pub fn obj_path(&self) -> &ObjPath {
        &self.obj_path
    }

    #[inline]
    pub fn field_name(&self) -> &FieldOrComponent {
        &self.field_name
    }

    #[inline]
    pub fn is_arrow(&self) -> bool {
        match self.field_name {
            FieldOrComponent::Field(_) => false,
            FieldOrComponent::Component(_) => true,
        }
    }
}

impl std::fmt::Display for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;
        self.obj_path.fmt(f)?;
        f.write_char('.')?;
        self.field_name.fmt(f)
    }
}
