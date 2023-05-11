use re_log_types::EntityPath;

#[cfg(feature = "serde")]
use crate::EditableAutoValue;

// ----------------------------------------------------------------------------

/// Properties for a collection of entities.
#[cfg(feature = "serde")]
#[derive(Clone, Default, PartialEq)]
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

// TODO(#1423): We need to properly split entity properties that only apply to specific
// views/primitives.
#[cfg(feature = "serde")]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct EntityProperties {
    pub visible: bool,
    pub visible_history: ExtraQueryHistory,
    pub interactive: bool,

    /// What kind of color mapping should be applied (none, map, texture, transfer..)?
    pub color_mapper: EditableAutoValue<ColorMapper>,

    /// Distance of the projection plane (frustum far plane).
    ///
    /// Only applies to pinhole cameras when in a spatial view, using 3D navigation.
    pub pinhole_image_plane_distance: EditableAutoValue<f32>,

    /// Should the depth texture be backprojected into a point cloud?
    ///
    /// Only applies to tensors with meaning=depth that are affected by a pinhole transform.
    ///
    /// The default for 3D views is `true`, but for 2D views it is `false`.
    pub backproject_depth: EditableAutoValue<bool>,

    /// How many depth units per world-space unit. e.g. 1000 for millimeters.
    ///
    /// This corresponds to [`re_log_types::component_types::Tensor::meter`].
    pub depth_from_world_scale: EditableAutoValue<f32>,

    /// Used to scale the radii of the points in the resulting point cloud.
    pub backproject_radius_scale: EditableAutoValue<f32>,
}

#[cfg(feature = "serde")]
impl Default for EntityProperties {
    fn default() -> Self {
        Self {
            visible: true,
            visible_history: ExtraQueryHistory::default(),
            interactive: true,
            color_mapper: EditableAutoValue::default(),
            pinhole_image_plane_distance: EditableAutoValue::default(),
            backproject_depth: EditableAutoValue::Auto(true),
            depth_from_world_scale: EditableAutoValue::default(),
            backproject_radius_scale: EditableAutoValue::Auto(1.0),
        }
    }
}

#[cfg(feature = "serde")]
impl EntityProperties {
    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        Self {
            visible: self.visible && child.visible,
            visible_history: self.visible_history.with_child(&child.visible_history),
            interactive: self.interactive && child.interactive,

            color_mapper: self.color_mapper.or(&child.color_mapper).clone(),

            pinhole_image_plane_distance: self
                .pinhole_image_plane_distance
                .or(&child.pinhole_image_plane_distance)
                .clone(),

            backproject_depth: self.backproject_depth.or(&child.backproject_depth).clone(),
            depth_from_world_scale: self
                .depth_from_world_scale
                .or(&child.depth_from_world_scale)
                .clone(),
            backproject_radius_scale: self
                .backproject_radius_scale
                .or(&child.backproject_radius_scale)
                .clone(),
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
} // ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Colormap {
    /// sRGB gray gradient = perceptually even
    Grayscale,

    Inferno,
    Magma,
    Plasma,
    #[default]
    Turbo,
    Viridis,
}

impl std::fmt::Display for Colormap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Colormap::Grayscale => "Grayscale",
            Colormap::Inferno => "Inferno",
            Colormap::Magma => "Magma",
            Colormap::Plasma => "Plasma",
            Colormap::Turbo => "Turbo",
            Colormap::Viridis => "Viridis",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ColorMapper {
    /// Use a well-known color map, pre-implemented as a wgsl module.
    Colormap(Colormap),
    // TODO(cmc): support textures.
    // TODO(cmc): support custom transfer functions.
}

impl std::fmt::Display for ColorMapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorMapper::Colormap(colormap) => colormap.fmt(f),
        }
    }
}

impl Default for ColorMapper {
    #[inline]
    fn default() -> Self {
        Self::Colormap(Colormap::default())
    }
}
