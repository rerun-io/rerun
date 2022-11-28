use re_log_types::ObjPath;

use crate::ObjectTree;

/// Stores a visibility toggle for a tree.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ObjectTreeProperties {
    /// Individual settings. Mutate this.
    pub individual: ObjectsProperties,

    /// Properties, as inherited from parent. Read from this.
    ///
    /// Recalculated at the start of each frame from [`Self::individual`].
    #[serde(skip)]
    pub projected: ObjectsProperties,
}

impl ObjectTreeProperties {
    pub fn on_frame_start(&mut self, object_tree: &ObjectTree) {
        crate::profile_function!();

        // NOTE(emilk): we could do this projection only when the object properties changes
        // and/or when new object paths are added, but such memoization would add complexity,
        // and in most cases this is pretty fast already.

        fn project_tree(
            tree_props: &mut ObjectTreeProperties,
            parent_properties: ObjectProps,
            tree: &ObjectTree,
        ) {
            let prop = parent_properties.with_child(&tree_props.individual.get(&tree.path));
            tree_props.projected.set(tree.path.clone(), prop);

            for child in tree.children.values() {
                project_tree(tree_props, prop, child);
            }
        }

        project_tree(self, ObjectProps::default(), object_tree);
    }
}

// ----------------------------------------------------------------------------

/// Properties for a tree of objects.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ObjectProps {
    pub visible: bool,
    pub extra_history: ExtraQueryHistory,
}

impl Default for ObjectProps {
    fn default() -> Self {
        Self {
            visible: true,
            extra_history: ExtraQueryHistory::default(),
        }
    }
}

impl ObjectProps {
    /// Multiply/and these together.
    fn with_child(&self, child: &Self) -> Self {
        Self {
            visible: self.visible && child.visible,
            extra_history: self.extra_history.with_child(&child.extra_history),
        }
    }
}

// ----------------------------------------------------------------------------

/// When showing an object in the history view, add this much history to it.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ExtraQueryHistory {
    /// Zero = off.
    pub nanos: u64,

    /// Zero = off.
    pub sequences: u64,
}

impl ExtraQueryHistory {
    /// Multiply/and these together.
    fn with_child(&self, child: &Self) -> Self {
        Self {
            nanos: self.nanos.max(child.nanos),
            sequences: self.sequences.max(child.sequences),
        }
    }
}
