use std::fmt::Formatter;

#[cfg(feature = "serde")]
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
    #[inline]
    pub fn get(&self, entity_path: &EntityPath) -> EntityProperties {
        self.props.get(entity_path).cloned().unwrap_or_default()
    }

    #[inline]
    pub fn get_opt(&self, entity_path: &EntityPath) -> Option<&EntityProperties> {
        self.props.get(entity_path)
    }

    /// Updates the properties for a given entity path.
    ///
    /// If an existing value is already in the map for the given entity path, the new value is merged
    /// with the existing value. When merging, auto values that were already set inside the map are
    /// preserved.
    #[inline]
    pub fn update(&mut self, entity_path: EntityPath, prop: EntityProperties) {
        if prop == EntityProperties::default() {
            self.props.remove(&entity_path); // save space
        } else if self.props.contains_key(&entity_path) {
            let merged = self
                .props
                .get(&entity_path)
                .cloned()
                .unwrap_or_default()
                .merge_with(&prop);
            self.props.insert(entity_path, merged);
        } else {
            self.props.insert(entity_path, prop);
        }
    }

    /// Overrides the properties for a given entity path.
    ///
    /// Like `update`, but auto properties are always updated.
    pub fn overwrite_properties(&mut self, entity_path: EntityPath, prop: EntityProperties) {
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

#[cfg(feature = "serde")]
impl FromIterator<(EntityPath, EntityProperties)> for EntityPropertyMap {
    fn from_iter<T: IntoIterator<Item = (EntityPath, EntityProperties)>>(iter: T) -> Self {
        Self {
            props: iter.into_iter().collect(),
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "serde")]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct EntityProperties {
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

    /// Should the legend be shown (for plot space views).
    pub show_legend: EditableAutoValue<bool>,

    /// The location of the legend (for plot space views).
    ///
    /// This is an Option instead of an EditableAutoValue to let each space view class decide on
    /// what's the best default.
    pub legend_location: Option<LegendCorner>,

    /// What kind of data aggregation to perform (for plot space views).
    pub time_series_aggregator: EditableAutoValue<TimeSeriesAggregator>,
}

#[cfg(feature = "serde")]
impl Default for EntityProperties {
    fn default() -> Self {
        Self {
            interactive: true,
            color_mapper: EditableAutoValue::default(),
            pinhole_image_plane_distance: EditableAutoValue::Auto(1.0),
            backproject_depth: EditableAutoValue::Auto(true),
            depth_from_world_scale: EditableAutoValue::Auto(1.0),
            backproject_radius_scale: EditableAutoValue::Auto(1.0),
            transform_3d_visible: EditableAutoValue::Auto(false),
            transform_3d_size: EditableAutoValue::Auto(1.0),
            show_legend: EditableAutoValue::Auto(true),
            legend_location: None,
            time_series_aggregator: EditableAutoValue::Auto(TimeSeriesAggregator::default()),
        }
    }
}

#[cfg(feature = "serde")]
impl EntityProperties {
    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        Self {
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

            show_legend: self.show_legend.or(&child.show_legend).clone(),
            legend_location: self.legend_location.or(child.legend_location),
            time_series_aggregator: self
                .time_series_aggregator
                .or(&child.time_series_aggregator)
                .clone(),
        }
    }

    /// Merge this `EntityProperty` with the values from another `EntityProperty`.
    ///
    /// When merging, other values are preferred over self values unless they are auto
    /// values, in which case self values are preferred.
    ///
    /// This is important to combine the base-layer of up-to-date auto-values with values
    /// loaded from the Blueprint store where the Auto values are not up-to-date.
    pub fn merge_with(&self, other: &Self) -> Self {
        Self {
            interactive: other.interactive,

            color_mapper: other.color_mapper.or(&self.color_mapper).clone(),

            pinhole_image_plane_distance: other
                .pinhole_image_plane_distance
                .or(&self.pinhole_image_plane_distance)
                .clone(),

            backproject_depth: other.backproject_depth.or(&self.backproject_depth).clone(),
            depth_from_world_scale: other
                .depth_from_world_scale
                .or(&self.depth_from_world_scale)
                .clone(),
            backproject_radius_scale: other
                .backproject_radius_scale
                .or(&self.backproject_radius_scale)
                .clone(),

            transform_3d_visible: other
                .transform_3d_visible
                .or(&self.transform_3d_visible)
                .clone(),
            transform_3d_size: self.transform_3d_size.or(&other.transform_3d_size).clone(),

            show_legend: other.show_legend.or(&self.show_legend).clone(),
            legend_location: other.legend_location.or(self.legend_location),
            time_series_aggregator: other
                .time_series_aggregator
                .or(&self.time_series_aggregator)
                .clone(),
        }
    }

    /// Determine whether this `EntityProperty` has user-edits relative to another `EntityProperty`
    pub fn has_edits(&self, other: &Self) -> bool {
        let Self {
            interactive,
            color_mapper,
            pinhole_image_plane_distance,
            backproject_depth,
            depth_from_world_scale,
            backproject_radius_scale,
            transform_3d_visible,
            transform_3d_size,
            show_legend,
            legend_location,
            time_series_aggregator,
        } = self;

        interactive != &other.interactive
            || color_mapper.has_edits(&other.color_mapper)
            || pinhole_image_plane_distance.has_edits(&other.pinhole_image_plane_distance)
            || backproject_depth.has_edits(&other.backproject_depth)
            || depth_from_world_scale.has_edits(&other.depth_from_world_scale)
            || backproject_radius_scale.has_edits(&other.backproject_radius_scale)
            || transform_3d_visible.has_edits(&other.transform_3d_visible)
            || transform_3d_size.has_edits(&other.transform_3d_size)
            || show_legend.has_edits(&other.show_legend)
            || *legend_location != other.legend_location
            || time_series_aggregator.has_edits(&other.time_series_aggregator)
    }
}

// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------

/// Where to put the legend?
///
/// This type duplicates `egui_plot::Corner` to add serialization support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum LegendCorner {
    LeftTop,
    RightTop,
    LeftBottom,
    RightBottom,
}

impl std::fmt::Display for LegendCorner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LegendCorner::LeftTop => write!(f, "Top Left"),
            LegendCorner::RightTop => write!(f, "Top Right"),
            LegendCorner::LeftBottom => write!(f, "Bottom Left"),
            LegendCorner::RightBottom => write!(f, "Bottom Right"),
        }
    }
}

// ----------------------------------------------------------------------------

/// What kind of aggregation should be performed when the zoom-level on the X axis goes below 1.0?
///
/// Aggregation affects the points' values and radii.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeSeriesAggregator {
    /// No aggregation.
    Off,

    /// Average all points in the range together.
    Average,

    /// Keep only the maximum values in the range.
    Max,

    /// Keep only the minimum values in the range.
    Min,

    /// Keep both the minimum and maximum values in the range.
    ///
    /// This will yield two aggregated points instead of one, effectively creating a vertical line.
    #[default]
    MinMax,

    /// Find both the minimum and maximum values in the range, then use the average of those.
    MinMaxAverage,
}

impl std::fmt::Display for TimeSeriesAggregator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeSeriesAggregator::Off => write!(f, "Off"),
            TimeSeriesAggregator::Average => write!(f, "Average"),
            TimeSeriesAggregator::Max => write!(f, "Max"),
            TimeSeriesAggregator::Min => write!(f, "Min"),
            TimeSeriesAggregator::MinMax => write!(f, "MinMax"),
            TimeSeriesAggregator::MinMaxAverage => write!(f, "MinMaxAverage"),
        }
    }
}

impl TimeSeriesAggregator {
    #[inline]
    pub fn variants() -> [TimeSeriesAggregator; 6] {
        // Just making sure this method won't compile if the enum gets modified.
        #[allow(clippy::match_same_arms)]
        match Self::default() {
            TimeSeriesAggregator::Off => {}
            TimeSeriesAggregator::Average => {}
            TimeSeriesAggregator::Max => {}
            TimeSeriesAggregator::Min => {}
            TimeSeriesAggregator::MinMax => {}
            TimeSeriesAggregator::MinMaxAverage => {}
        }

        [
            TimeSeriesAggregator::Off,
            TimeSeriesAggregator::Average,
            TimeSeriesAggregator::Max,
            TimeSeriesAggregator::Min,
            TimeSeriesAggregator::MinMax,
            TimeSeriesAggregator::MinMaxAverage,
        ]
    }

    #[inline]
    pub fn description(&self) -> &'static str {
        match self {
            TimeSeriesAggregator::Off => "No aggregation.",
            TimeSeriesAggregator::Average => "Average all points in the range together.",
            TimeSeriesAggregator::Max => "Keep only the maximum values in the range.",
            TimeSeriesAggregator::Min => "Keep only the minimum values in the range.",
            TimeSeriesAggregator::MinMax => "Keep both the minimum and maximum values in the range.\nThis will yield two aggregated points instead of one, effectively creating a vertical line.",
            TimeSeriesAggregator::MinMaxAverage => "Find both the minimum and maximum values in the range, then use the average of those",
        }
    }
}
