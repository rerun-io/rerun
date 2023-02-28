use re_arrow_store::LatestAtQuery;
use re_log_types::{
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator,
    msg_bundle::DeserializableComponent, EntityPath,
};

use crate::{log_db::EntityDb, EditableAutoValue};

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

    /// Distance of the projection plane (frustum far plane).
    ///
    /// Only applies to pinhole cameras when in a spatial view, using 3D navigation.
    pub pinhole_image_plane_distance: EditableAutoValue<ordered_float::NotNan<f32>>,
}

impl EntityProperties {
    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        Self {
            visible: self.visible && child.visible,
            visible_history: self.visible_history.with_child(&child.visible_history),
            interactive: self.interactive && child.interactive,
            // TODO(andreas): Inheriting doesn't work properly since parents don't set this value usually
            pinhole_image_plane_distance: child.pinhole_image_plane_distance.clone(),
        }
    }
}

impl Default for EntityProperties {
    fn default() -> Self {
        Self {
            visible: true,
            visible_history: ExtraQueryHistory::default(),
            interactive: true,
            pinhole_image_plane_distance: EditableAutoValue::default(),
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

/// Get the latest value for a given [`re_log_types::msg_bundle::Component`].
///
/// This assumes that the row we get from the store only contains a single instance for this
/// component; it will log a warning otherwise.
pub fn query_latest_single<C: DeserializableComponent>(
    entity_db: &EntityDb,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<C>
where
    for<'b> &'b C::ArrayType: IntoIterator,
{
    crate::profile_function!();

    // Although it would be nice to use the `re_query` helpers for this, we would need to move
    // this out of re_data_store to avoid a circular dep. Since we don't need to do a join for
    // single components this is easy enough.
    let data_store = &entity_db.data_store;

    let components = [C::name()];

    let row_indices = data_store.latest_at(query, entity_path, C::name(), &components)?;

    let results = data_store.get(&components, &row_indices);
    let arr = results.get(0)?.as_ref()?.as_ref();

    let mut iter = arrow_array_deserialize_iterator::<C>(arr).ok()?;

    let component = iter.next();

    if iter.next().is_some() {
        re_log::warn_once!("Unexpected batch for {} at: {}", C::name(), entity_path);
    }

    component
}
