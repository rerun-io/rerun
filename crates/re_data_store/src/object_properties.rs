use re_arrow_store::{LatestAtQuery, TimeInt, Timeline};
use re_log_types::{
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator, msg_bundle::Component,
    FieldName, ObjPath, Transform,
};

use crate::log_db::ObjDb;

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
                    .unwrap_or_else(|| pinhole.focal_length_in_pixels().y());
                ordered_float::NotNan::new(distance).unwrap_or_default()
            })
            .into()
    }

    /// see `pinhole_image_plane_distance()`
    pub fn set_pinhole_image_plane_distance(&mut self, distance: f32) {
        self.pinhole_image_plane_distance = ordered_float::NotNan::new(distance).ok();
    }

    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        Self {
            visible: self.visible && child.visible,
            visible_history: self.visible_history.with_child(&child.visible_history),
            interactive: self.interactive && child.interactive,
            pinhole_image_plane_distance: child
                .pinhole_image_plane_distance
                .or(self.pinhole_image_plane_distance),
        }
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

impl ExtraQueryHistory {
    /// Multiply/and these together.
    fn with_child(&self, child: &Self) -> Self {
        Self {
            nanos: self.nanos.max(child.nanos),
            sequences: self.sequences.max(child.sequences),
        }
    }
}

// ----------------------------------------------------------------------------

/// Get the latest value of the `_transform` meta-field of the given object.
fn query_transform_classic(
    obj_db: &ObjDb,
    timeline: &Timeline,
    obj_path: &ObjPath,
    query_time: Option<i64>,
) -> Option<Transform> {
    let store = obj_db.store.get(timeline)?;

    let field_store = store.get(obj_path)?.get(&FieldName::from("_transform"))?;
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

/// Get the latest value of the `Transform` component from arrow
fn query_transform_arrow(
    obj_db: &ObjDb,
    timeline: &Timeline,
    ent_path: &ObjPath,
    query_time: Option<i64>,
) -> Option<Transform> {
    // Although it would be nice to use the `re_query` helpers for this, we would need to move
    // this out of re_data_store to avoid a circular dep. Since we don't need to do a join for
    // transforms this is easy enough.
    let arrow_store = &obj_db.arrow_store;

    let query = LatestAtQuery::new(*timeline, TimeInt::from(query_time?));

    let components = [Transform::name()];

    let row_indices = arrow_store.latest_at(&query, ent_path, Transform::name(), &components)?;

    let results = arrow_store.get(&components, &row_indices);
    let arr = results.get(0)?.as_ref()?.as_ref();

    let mut iter = arrow_array_deserialize_iterator::<Transform>(arr).ok()?;

    let transform = iter.next();

    if iter.next().is_some() {
        re_log::warn_once!("Unexpected batch for Transform at: {}", ent_path);
    }

    transform
}

/// Get the latest value of the transform
///
/// We first look for the transform in the classic storage system since that's
/// what most users are still using. If we don't find the transform there, then
/// we check to see if it exists in the arrow storage.
pub fn query_transform(
    obj_db: &ObjDb,
    timeline: &Timeline,
    obj_path: &ObjPath,
    query_time: Option<i64>,
) -> Option<Transform> {
    query_transform_classic(obj_db, timeline, obj_path, query_time)
        .or_else(|| query_transform_arrow(obj_db, timeline, obj_path, query_time))
}
