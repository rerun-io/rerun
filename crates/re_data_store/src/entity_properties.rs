use re_arrow_store::LatestAtQuery;
use re_log_types::{
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator, msg_bundle::Component,
    EntityPath, Transform,
};

use crate::log_db::EntityDb;

// ----------------------------------------------------------------------------

/// Properties for a collection of entities.
#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPropertyMap {
    props: nohash_hasher::IntMap<EntityPath, EntityProperties>,
}

impl EntityPropertyMap {
    pub fn get(&self, entity_path: &EntityPath) -> EntityProperties {
        self.props.get(entity_path).cloned().unwrap_or_default()
    }

    pub fn set(&mut self, entity_path: EntityPath, prop: EntityProperties) {
        if prop == EntityProperties::default() {
            self.props.remove(&entity_path); // save space
        } else {
            self.props.insert(entity_path, prop);
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct EntityProperties {
    pub visible: bool,
    pub visible_history: ExtraQueryHistory,
    pub interactive: bool,
    pinhole_image_plane_distance: Option<ordered_float::NotNan<f32>>,
}

impl EntityProperties {
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

impl Default for EntityProperties {
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

/// When showing an entity in the history view, add this much history to it.
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

/// Get the latest value of the transform
///
/// We first look for the transform in the classic storage system since that's
/// what most users are still using. If we don't find the transform there, then
/// we check to see if it exists in the arrow storage.
pub fn query_transform(
    entity_db: &EntityDb,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<Transform> {
    crate::profile_function!();

    // Although it would be nice to use the `re_query` helpers for this, we would need to move
    // this out of re_data_store to avoid a circular dep. Since we don't need to do a join for
    // transforms this is easy enough.
    let data_store = &entity_db.data_store;

    let components = [Transform::name()];

    let row_indices = data_store.latest_at(query, entity_path, Transform::name(), &components)?;

    let results = data_store.get(&components, &row_indices);
    let arr = results.get(0)?.as_ref()?.as_ref();

    let mut iter = arrow_array_deserialize_iterator::<Transform>(arr).ok()?;

    let transform = iter.next();

    if iter.next().is_some() {
        re_log::warn_once!("Unexpected batch for Transform at: {}", entity_path);
    }

    transform
}
