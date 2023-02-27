use re_arrow_store::LatestAtQuery;
use re_log_types::{
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator,
    msg_bundle::DeserializableComponent, EntityPath,
};

use crate::log_db::EntityDb;

#[cfg(feature = "serde")]
use crate::EditableAutoValue;

// ----------------------------------------------------------------------------

/// Properties for a collection of entities.
#[cfg(feature = "serde")]
#[derive(Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPropertyMap {
    props: nohash_hasher::IntMap<EntityPath, EntityProperties>,
}

#[cfg(feature = "serde")]
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

// TODO(cmc): We need to properly split entity properties that only apply to specific
// views/primitives.
#[cfg(feature = "serde")]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct EntityProperties {
    pub visible: bool,
    pub visible_history: ExtraQueryHistory,
    pub interactive: bool,

    /// Distance of the projection plane (frustum far plane).
    ///
    /// Only applies to pinhole cameras when in a spatial view, using 3D navigation.
    ///
    pub pinhole_image_plane_distance: EditableAutoValue<f32>,

    /// Should the depth texture be backprojected into a point cloud?
    ///
    /// Only applies to tensors with meaning=depth that are affected by a pinhole transform when
    /// in a spatial view, using 3D navigation.
    pub backproject_depth: bool,
    /// Entity path of the pinhole transform used for the backprojection.
    ///
    /// `None` means backprojection is disabled.
    pub backproject_pinhole_ent_path: Option<EntityPath>,
    /// Used to scale the resulting point cloud.
    pub backproject_scale: EditableAutoValue<f32>,
    /// Used to scale the radii of the points in the resulting point cloud.
    pub backproject_radius_scale: EditableAutoValue<f32>,
}

#[cfg(feature = "serde")]
impl EntityProperties {
    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        Self {
            visible: self.visible && child.visible,
            visible_history: self.visible_history.with_child(&child.visible_history),
            interactive: self.interactive && child.interactive,
            pinhole_image_plane_distance: self
                .pinhole_image_plane_distance
                .or(&child.pinhole_image_plane_distance)
                .clone(),
            backproject_depth: self.backproject_depth || child.backproject_depth,
            backproject_pinhole_ent_path: child
                .backproject_pinhole_ent_path
                .clone()
                .or(self.backproject_pinhole_ent_path.clone()),
            backproject_scale: child.backproject_scale.or(&self.backproject_scale).clone(),
            backproject_radius_scale: child
                .backproject_radius_scale
                .or(&self.backproject_radius_scale)
                .clone(),
        }
    }
}

#[cfg(feature = "serde")]
impl Default for EntityProperties {
    fn default() -> Self {
        Self {
            visible: true,
            visible_history: ExtraQueryHistory::default(),
            interactive: true,
            pinhole_image_plane_distance: EditableAutoValue::default(),
            backproject_depth: true,
            backproject_pinhole_ent_path: None,
            backproject_scale: EditableAutoValue::default(),
            backproject_radius_scale: EditableAutoValue::default(),
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
    #[allow(dead_code)]
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
