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
    // TODO(#5067): Test property used so we don't have to continuously adjust existing tests while we're dismantling `EntityProperties`.
    pub test_property: bool,

    /// Should the depth texture be backprojected into a point cloud?
    ///
    /// Only applies to tensors with meaning=depth that are affected by a pinhole transform.
    ///
    /// The default for 3D views is `true`, but for 2D views it is `false`.
    pub backproject_depth: EditableAutoValue<bool>, // TODO(andreas): should be a component on the DepthImage archetype.

    /// How many depth units per world-space unit. e.g. 1000 for millimeters.
    ///
    /// This corresponds to `re_components::Tensor::meter`.
    pub depth_from_world_scale: EditableAutoValue<f32>, // TODO(andreas): Just remove once we can edit meter & be able to set semi-clever defaults per visualizer.

    /// Used to scale the radii of the points in the resulting point cloud.
    pub backproject_radius_scale: EditableAutoValue<f32>, // TODO(andreas): should be a component on the DepthImage archetype.

    /// What kind of data aggregation to perform (for plot space views).
    pub time_series_aggregator: EditableAutoValue<TimeSeriesAggregator>, // TODO(andreas): Should be a component probably on SeriesLine, but today it would become a view property.
}

#[cfg(feature = "serde")]
impl Default for EntityProperties {
    fn default() -> Self {
        Self {
            test_property: true,
            backproject_depth: EditableAutoValue::Auto(true),
            depth_from_world_scale: EditableAutoValue::Auto(1.0),
            backproject_radius_scale: EditableAutoValue::Auto(1.0),
            time_series_aggregator: EditableAutoValue::Auto(TimeSeriesAggregator::default()),
        }
    }
}

#[cfg(feature = "serde")]
impl EntityProperties {
    /// Multiply/and these together.
    pub fn with_child(&self, child: &Self) -> Self {
        Self {
            test_property: self.test_property && child.test_property,

            backproject_depth: self.backproject_depth.or(&child.backproject_depth).clone(),
            depth_from_world_scale: self
                .depth_from_world_scale
                .or(&child.depth_from_world_scale)
                .clone(),
            backproject_radius_scale: self
                .backproject_radius_scale
                .or(&child.backproject_radius_scale)
                .clone(),

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
            test_property: other.test_property,

            backproject_depth: other.backproject_depth.or(&self.backproject_depth).clone(),
            depth_from_world_scale: other
                .depth_from_world_scale
                .or(&self.depth_from_world_scale)
                .clone(),
            backproject_radius_scale: other
                .backproject_radius_scale
                .or(&self.backproject_radius_scale)
                .clone(),

            time_series_aggregator: other
                .time_series_aggregator
                .or(&self.time_series_aggregator)
                .clone(),
        }
    }

    /// Determine whether this `EntityProperty` has user-edits relative to another `EntityProperty`
    pub fn has_edits(&self, other: &Self) -> bool {
        let Self {
            test_property,
            backproject_depth,
            depth_from_world_scale,
            backproject_radius_scale,
            time_series_aggregator,
        } = self;

        test_property != &other.test_property
            || backproject_depth.has_edits(&other.backproject_depth)
            || depth_from_world_scale.has_edits(&other.depth_from_world_scale)
            || backproject_radius_scale.has_edits(&other.backproject_radius_scale)
            || time_series_aggregator.has_edits(&other.time_series_aggregator)
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
            Self::Off => write!(f, "Off"),
            Self::Average => write!(f, "Average"),
            Self::Max => write!(f, "Max"),
            Self::Min => write!(f, "Min"),
            Self::MinMax => write!(f, "MinMax"),
            Self::MinMaxAverage => write!(f, "MinMaxAverage"),
        }
    }
}

impl TimeSeriesAggregator {
    #[inline]
    pub fn variants() -> [Self; 6] {
        // Just making sure this method won't compile if the enum gets modified.
        #[allow(clippy::match_same_arms)]
        match Self::default() {
            Self::Off => {}
            Self::Average => {}
            Self::Max => {}
            Self::Min => {}
            Self::MinMax => {}
            Self::MinMaxAverage => {}
        }

        [
            Self::Off,
            Self::Average,
            Self::Max,
            Self::Min,
            Self::MinMax,
            Self::MinMaxAverage,
        ]
    }

    #[inline]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Off => "No aggregation.",
            Self::Average => "Average all points in the range together.",
            Self::Max => "Keep only the maximum values in the range.",
            Self::Min => "Keep only the minimum values in the range.",
            Self::MinMax => "Keep both the minimum and maximum values in the range.\nThis will yield two aggregated points instead of one, effectively creating a vertical line.",
            Self::MinMaxAverage => "Find both the minimum and maximum values in the range, then use the average of those",
        }
    }
}
