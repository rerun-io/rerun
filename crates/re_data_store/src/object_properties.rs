use re_log_types::ObjPath;

// ----------------------------------------------------------------------------

/// Properties for a tree of objects.
#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ObjectsProperties {
    props: nohash_hasher::IntMap<ObjPath, ObjectProps>,
}

impl ObjectsProperties {
    pub fn get(&self, obj_path: &ObjPath) -> ObjectProps {
        self.props.get(obj_path).copied().unwrap_or_default()
    }

    pub fn set(&mut self, obj_path: ObjPath, prop: ObjectProps) {
        if prop == ObjectProps::default() {
            self.props.remove(&obj_path); // save space
        } else {
            self.props.insert(obj_path, prop);
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ObjectProps {
    pub visible: bool,
    pub visible_history: ExtraQueryHistory,
    pub interactive: bool,
}

impl Default for ObjectProps {
    fn default() -> Self {
        Self {
            visible: true,
            visible_history: ExtraQueryHistory::default(),
            interactive: true,
        }
    }
}

// ----------------------------------------------------------------------------

/// When showing an object in the history view, add this much history to it.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ExtraQueryHistory {
    /// Zero = off.
    pub nanos: i64,

    /// Zero = off.
    pub sequences: i64,
}
