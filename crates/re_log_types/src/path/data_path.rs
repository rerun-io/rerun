use crate::path::{FieldName, ObjPath};

/// `camera / "left" / points / #42` / "pos"
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataPath {
    /// `camera / "left" / points / #42`
    pub obj_path: ObjPath,

    /// "pos"
    pub field_name: FieldName,
}

impl DataPath {
    #[inline]
    pub fn new(obj_path: ObjPath, field_name: FieldName) -> Self {
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
    pub fn field_name(&self) -> &FieldName {
        &self.field_name
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
