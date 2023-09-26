// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/depth_image.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// A depth image.
///
/// The shape of the `TensorData` must be mappable to an `HxW` tensor.
/// Each pixel corresponds to a depth value in units specified by `meter`.
///
/// ## Example
///
/// ```ignore
/// //! Create and log a depth image.
///
/// use ndarray::{s, Array, ShapeBuilder};
/// use rerun::{archetypes::DepthImage, RecordingStreamBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;
///
///     let mut image = Array::<u16, _>::from_elem((8, 12).f(), 65535);
///     image.slice_mut(s![0..4, 0..6]).fill(20000);
///     image.slice_mut(s![4..8, 6..12]).fill(45000);
///
///     let depth_image = DepthImage::try_from(image)?.with_meter(10_000.0);
///
///     rec.log("depth", &depth_image)?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct DepthImage {
    /// The depth-image data. Should always be a rank-2 tensor.
    pub data: crate::components::TensorData,

    /// An optional floating point value that specifies how long a meter is in the native depth units.
    ///
    /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
    /// and a range of up to ~65 meters (2^16 / 1000).
    pub meter: Option<crate::components::DepthMeter>,

    /// An optional floating point value that specifies the 2D drawing order.
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<crate::components::DrawOrder>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TensorData".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.DepthImageIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.DepthMeter".into(),
            "rerun.components.DrawOrder".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.TensorData".into(),
            "rerun.components.DepthImageIndicator".into(),
            "rerun.components.DepthMeter".into(),
            "rerun.components.DrawOrder".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl DepthImage {
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`DepthImage`] [`crate::Archetype`]
pub type DepthImageIndicator = crate::GenericIndicatorComponent<DepthImage>;

impl crate::Archetype for DepthImage {
    type Indicator = DepthImageIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.DepthImage".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: DepthImageIndicator = DepthImageIndicator::DEFAULT;
        crate::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn try_from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let data = {
            let array = arrays_by_name
                .get("data")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.DepthImage#data")?;
            <crate::components::TensorData>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.DepthImage#data")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.DepthImage#data")?
        };
        let meter = if let Some(array) = arrays_by_name.get("meter") {
            Some({
                <crate::components::DepthMeter>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.DepthImage#meter")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.DepthImage#meter")?
            })
        } else {
            None
        };
        let draw_order = if let Some(array) = arrays_by_name.get("draw_order") {
            Some({
                <crate::components::DrawOrder>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.DepthImage#draw_order")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.DepthImage#draw_order")?
            })
        } else {
            None
        };
        Ok(Self {
            data,
            meter,
            draw_order,
        })
    }
}

impl crate::AsComponents for DepthImage {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.data as &dyn crate::ComponentBatch).into()),
            self.meter
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.draw_order
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }
}

impl DepthImage {
    pub fn new(data: impl Into<crate::components::TensorData>) -> Self {
        Self {
            data: data.into(),
            meter: None,
            draw_order: None,
        }
    }

    pub fn with_meter(mut self, meter: impl Into<crate::components::DepthMeter>) -> Self {
        self.meter = Some(meter.into());
        self
    }

    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = Some(draw_order.into());
        self
    }
}
