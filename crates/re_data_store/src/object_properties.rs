use re_log_types::{FieldName, ObjPath, Transform};

// ----------------------------------------------------------------------------

/// Properties for a collection of objects.
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
    pinhole_image_plane_distance: Option<ordered_float::NotNan<f32>>,
}

impl ObjectProps {
    /// If this has a pinhole camera transform, how far away is the image plane.
    ///
    /// Scale relative to the respective space the pinhole camera is in.
    /// None indicates the user never edited this field (should use a meaningful default then).
    ///
    /// Method returns a pinhole camera specific default if the value hasn't been set yet.
    pub fn pinhole_image_plane_distance(&self, pinhole: &re_log_types::Pinhole) -> f32 {
        self.pinhole_image_plane_distance
            .unwrap_or_else(|| {
                let distance = pinhole
                    .focal_length()
                    .unwrap_or_else(|| pinhole.focal_length_in_pixels().y);
                ordered_float::NotNan::new(distance).unwrap_or_default()
            })
            .into()
    }

    /// see `pinhole_image_plane_distance()`
    pub fn set_pinhole_image_plane_distance(&mut self, distance: f32) {
        self.pinhole_image_plane_distance = ordered_float::NotNan::new(distance).ok();
    }
}

impl Default for ObjectProps {
    fn default() -> Self {
        Self {
            visible: true,
            visible_history: ExtraQueryHistory::default(),
            interactive: true,
            pinhole_image_plane_distance: None,
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

// ----------------------------------------------------------------------------

/// Get the latest value of the `_transform` meta-field of the given object.
pub fn query_transform(
    store: Option<&crate::TimelineStore<i64>>,
    obj_path: &ObjPath,
    query_time: Option<i64>,
) -> Option<Transform> {
    let field_store = store?.get(obj_path)?.get(&FieldName::from("_transform"))?;
    let mono_field_store = field_store.get_mono::<Transform>().ok()?;

    // There is a transform, at least at _some_ time.
    // Is there a transform _now_?
    let latest = query_time
        .and_then(|query_time| mono_field_store.latest_at(&query_time))
        .map(|(_, _, transform)| transform.clone());

    // If not, return an unknown transform to indicate that there is
    // still a space-split.
    Some(latest.unwrap_or(Transform::Unknown))
}
