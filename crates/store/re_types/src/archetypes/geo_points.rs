// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/geo_points.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Geospatial points with positions expressed in [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees), and optional colors and radii.
///
/// ## Example
///
/// ### Log a geospatial point
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_geo_points").spawn()?;
///
///     rec.log(
///         "rerun_hq",
///         &rerun::GeoPoints::from_lat_lon([(59.319221, 18.075631)])
///             .with_radii([rerun::Radius::new_ui_points(10.0)])
///             .with_colors([rerun::Color::from_rgb(255, 0, 0)]),
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/1200w.png">
///   <img src="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct GeoPoints {
    /// The [EPSG:4326](https://epsg.io/4326) coordinates for the points (North/East-positive degrees).
    pub positions: Vec<crate::components::LatLon>,

    /// Optional radii for the points, effectively turning them into circles.
    ///
    /// *Note*: scene units radiii are interpreted as meters.
    pub radii: Option<Vec<crate::components::Radius>>,

    /// Optional colors for the points.
    pub colors: Option<Vec<crate::components::Color>>,

    /// Optional class Ids for the points.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors if not specified explicitly.
    pub class_ids: Option<Vec<crate::components::ClassId>>,
}

impl GeoPoints {
    /// Returns the [`ComponentDescriptor`] for [`Self::positions`].
    #[inline]
    pub fn descriptor_positions() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.GeoPoints".into()),
            component_name: "rerun.components.LatLon".into(),
            archetype_field_name: Some("positions".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::radii`].
    #[inline]
    pub fn descriptor_radii() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.GeoPoints".into()),
            component_name: "rerun.components.Radius".into(),
            archetype_field_name: Some("radii".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::colors`].
    #[inline]
    pub fn descriptor_colors() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.GeoPoints".into()),
            component_name: "rerun.components.Color".into(),
            archetype_field_name: Some("colors".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::class_ids`].
    #[inline]
    pub fn descriptor_class_ids() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.GeoPoints".into()),
            component_name: "rerun.components.ClassId".into(),
            archetype_field_name: Some("class_ids".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.GeoPoints".into()),
            component_name: "rerun.components.GeoPointsIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [GeoPoints::descriptor_positions()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            GeoPoints::descriptor_radii(),
            GeoPoints::descriptor_colors(),
            GeoPoints::descriptor_indicator(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [GeoPoints::descriptor_class_ids()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            GeoPoints::descriptor_positions(),
            GeoPoints::descriptor_radii(),
            GeoPoints::descriptor_colors(),
            GeoPoints::descriptor_indicator(),
            GeoPoints::descriptor_class_ids(),
        ]
    });

impl GeoPoints {
    /// The total number of components in the archetype: 1 required, 3 recommended, 1 optional
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`GeoPoints`] [`::re_types_core::Archetype`]
pub type GeoPointsIndicator = ::re_types_core::GenericIndicatorComponent<GeoPoints>;

impl ::re_types_core::Archetype for GeoPoints {
    type Indicator = GeoPointsIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.GeoPoints".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Geo points"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: GeoPointsIndicator = GeoPointsIndicator::DEFAULT;
        ComponentBatchCowWithDescriptor::new(&INDICATOR as &dyn ::re_types_core::ComponentBatch)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentDescriptor, arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_descr: ::nohash_hasher::IntMap<_, _> = arrow_data.into_iter().collect();
        let positions = {
            let array = arrays_by_descr
                .get(&Self::descriptor_positions())
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.GeoPoints#positions")?;
            <crate::components::LatLon>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.GeoPoints#positions")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.GeoPoints#positions")?
        };
        let radii = if let Some(array) = arrays_by_descr.get(&Self::descriptor_radii()) {
            Some({
                <crate::components::Radius>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.GeoPoints#radii")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.GeoPoints#radii")?
            })
        } else {
            None
        };
        let colors = if let Some(array) = arrays_by_descr.get(&Self::descriptor_colors()) {
            Some({
                <crate::components::Color>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.GeoPoints#colors")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.GeoPoints#colors")?
            })
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_descr.get(&Self::descriptor_class_ids()) {
            Some({
                <crate::components::ClassId>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.GeoPoints#class_ids")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.GeoPoints#class_ids")?
            })
        } else {
            None
        };
        Ok(Self {
            positions,
            radii,
            colors,
            class_ids,
        })
    }
}

impl ::re_types_core::AsComponents for GeoPoints {
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            (Some(&self.positions as &dyn ComponentBatch)).map(|batch| {
                ::re_types_core::ComponentBatchCowWithDescriptor {
                    batch: batch.into(),
                    descriptor_override: Some(Self::descriptor_positions()),
                }
            }),
            (self
                .radii
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_radii()),
            }),
            (self
                .colors
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_colors()),
            }),
            (self
                .class_ids
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_class_ids()),
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for GeoPoints {}

impl GeoPoints {
    /// Create a new `GeoPoints`.
    #[inline]
    pub(crate) fn new(
        positions: impl IntoIterator<Item = impl Into<crate::components::LatLon>>,
    ) -> Self {
        Self {
            positions: positions.into_iter().map(Into::into).collect(),
            radii: None,
            colors: None,
            class_ids: None,
        }
    }

    /// Optional radii for the points, effectively turning them into circles.
    ///
    /// *Note*: scene units radiii are interpreted as meters.
    #[inline]
    pub fn with_radii(
        mut self,
        radii: impl IntoIterator<Item = impl Into<crate::components::Radius>>,
    ) -> Self {
        self.radii = Some(radii.into_iter().map(Into::into).collect());
        self
    }

    /// Optional colors for the points.
    #[inline]
    pub fn with_colors(
        mut self,
        colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.colors = Some(colors.into_iter().map(Into::into).collect());
        self
    }

    /// Optional class Ids for the points.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors if not specified explicitly.
    #[inline]
    pub fn with_class_ids(
        mut self,
        class_ids: impl IntoIterator<Item = impl Into<crate::components::ClassId>>,
    ) -> Self {
        self.class_ids = Some(class_ids.into_iter().map(Into::into).collect());
        self
    }
}

impl ::re_byte_size::SizeBytes for GeoPoints {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.positions.heap_size_bytes()
            + self.radii.heap_size_bytes()
            + self.colors.heap_size_bytes()
            + self.class_ids.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::components::LatLon>>::is_pod()
            && <Option<Vec<crate::components::Radius>>>::is_pod()
            && <Option<Vec<crate::components::Color>>>::is_pod()
            && <Option<Vec<crate::components::ClassId>>>::is_pod()
    }
}
