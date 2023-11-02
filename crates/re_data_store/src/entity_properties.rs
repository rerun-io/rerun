#[cfg(feature = "serde")]
use re_log_types::EntityPath;
use re_log_types::TimeInt;

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
    #[inline]
    pub fn get(&self, entity_path: &EntityPath) -> EntityProperties {
        self.props.get(entity_path).cloned().unwrap_or_default()
    }

    #[inline]
    pub fn set(&mut self, entity_path: EntityPath, prop: EntityProperties) {
        if prop == EntityProperties::default() {
            self.props.remove(&entity_path); // save space
        } else {
            self.props.insert(entity_path, prop);
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&EntityPath, &EntityProperties)> {
        self.props.iter()
    }

    /// Determine whether this `EntityPropertyMap` has user-edits relative to another `EntityPropertyMap`
    pub fn has_edits(&self, other: &Self) -> bool {
        self.props.len() != other.props.len()
            || self.props.iter().any(|(key, val)| {
                other
                    .props
                    .get(key)
                    .map_or(true, |other_val| val.has_edits(other_val))
            })
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
    /// This corresponds to `re_components::Tensor::meter`.
    pub depth_from_world_scale: EditableAutoValue<f32>,

    /// Used to scale the radii of the points in the resulting point cloud.
    pub backproject_radius_scale: EditableAutoValue<f32>,

    /// Whether to show the 3D transform visualization at all.
    pub transform_3d_visible: EditableAutoValue<bool>,

    /// The length of the arrows in the entity's own coordinate system (space).
    pub transform_3d_size: EditableAutoValue<f32>,
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
            transform_3d_visible: EditableAutoValue::Auto(false),
            transform_3d_size: EditableAutoValue::Auto(1.0),
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

            transform_3d_visible: self
                .transform_3d_visible
                .or(&child.transform_3d_visible)
                .clone(),
            transform_3d_size: self.transform_3d_size.or(&child.transform_3d_size).clone(),
        }
    }

    /// Determine whether this `EntityProperty` has user-edits relative to another `EntityProperty`
    pub fn has_edits(&self, other: &Self) -> bool {
        let Self {
            visible,
            visible_history,
            interactive,
            color_mapper,
            pinhole_image_plane_distance,
            backproject_depth,
            depth_from_world_scale,
            backproject_radius_scale,
            transform_3d_visible,
            transform_3d_size,
        } = self;

        visible != &other.visible
            || visible_history != &other.visible_history
            || interactive != &other.interactive
            || color_mapper.has_edits(&other.color_mapper)
            || pinhole_image_plane_distance.has_edits(&other.pinhole_image_plane_distance)
            || backproject_depth.has_edits(&other.backproject_depth)
            || depth_from_world_scale.has_edits(&other.depth_from_world_scale)
            || backproject_radius_scale.has_edits(&other.backproject_radius_scale)
            || transform_3d_visible.has_edits(&other.transform_3d_visible)
            || transform_3d_size.has_edits(&other.transform_3d_size)
    }
}

// ----------------------------------------------------------------------------

/// One of the boundary of the visible history.
///
/// The for [`Relative`] and [`Absolute`], the value are either nanos or frames, depending on the
/// type of timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum VisibleHistoryBoundary {
    /// Boundary is a value relative to the time cursor
    Relative(i64),

    /// Boundary is an absolute value
    Absolute(i64),

    /// The boundary extends to infinity.
    Infinite,
}

impl VisibleHistoryBoundary {
    pub const AT_CURSOR: Self = Self::Relative(0);
}

impl Default for VisibleHistoryBoundary {
    fn default() -> Self {
        Self::AT_CURSOR
    }
}

/// Visible history bounds.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct VisibleHistory {
    /// Low time boundary.
    pub from: VisibleHistoryBoundary,

    /// High time boundary.
    pub to: VisibleHistoryBoundary,
}

impl VisibleHistory {
    /// Value with the visible history feature is disabled.
    pub const OFF: Self = Self {
        from: VisibleHistoryBoundary::AT_CURSOR,
        to: VisibleHistoryBoundary::AT_CURSOR,
    };

    pub fn from(&self, cursor: TimeInt) -> TimeInt {
        match self.from {
            VisibleHistoryBoundary::Absolute(value) => TimeInt::from(value),
            VisibleHistoryBoundary::Relative(value) => cursor + TimeInt::from(value),
            VisibleHistoryBoundary::Infinite => TimeInt::MIN,
        }
    }

    pub fn to(&self, cursor: TimeInt) -> TimeInt {
        match self.to {
            VisibleHistoryBoundary::Absolute(value) => TimeInt::from(value),
            VisibleHistoryBoundary::Relative(value) => cursor + TimeInt::from(value),
            VisibleHistoryBoundary::Infinite => TimeInt::MAX,
        }
    }
}

/// When showing an entity in the history view, add this much history to it.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ExtraQueryHistory {
    /// Is the feature enabled?
    pub enabled: bool,

    /// Visible history settings for time timelines
    pub nanos: VisibleHistory,

    /// Visible history settings for frame timelines
    pub sequences: VisibleHistory,
}

impl ExtraQueryHistory {
    /// Multiply/and these together.
    #[allow(dead_code)]
    fn with_child(&self, child: &Self) -> Self {
        if child.enabled {
            *child
        } else {
            *self
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
