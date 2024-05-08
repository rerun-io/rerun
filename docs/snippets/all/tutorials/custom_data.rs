//! Demonstrates how to implement custom archetypes and components, and extend existing ones.

use rerun::{
    demo_util::grid,
    external::{arrow2, glam, re_types},
};

// ---

/// A custom [component bundle] that extends Rerun's builtin [`rerun::Points3D`] archetype with extra
/// [`rerun::Component`]s.
///
/// [component bundle]: [`AsComponents`]
struct CustomPoints3D {
    points3d: rerun::Points3D,
    confidences: Option<Vec<Confidence>>,
}

impl rerun::AsComponents for CustomPoints3D {
    fn as_component_batches(&self) -> Vec<rerun::MaybeOwnedComponentBatch<'_>> {
        let indicator = rerun::NamedIndicatorComponent("user.CustomPoints3DIndicator".into());
        self.points3d
            .as_component_batches()
            .into_iter()
            .chain(
                [
                    Some(indicator.to_batch()),
                    self.confidences
                        .as_ref()
                        .map(|v| (v as &dyn rerun::ComponentBatch).into()),
                ]
                .into_iter()
                .flatten(),
            )
            .collect()
    }
}

// ---

/// A custom [`rerun::Component`] that is backed by a builtin [`rerun::Float32`] scalar [`rerun::Datatype`].
#[derive(Debug, Clone, Copy)]
struct Confidence(rerun::Float32);

impl From<f32> for Confidence {
    fn from(v: f32) -> Self {
        Self(rerun::Float32(v))
    }
}

impl rerun::SizeBytes for Confidence {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl rerun::Loggable for Confidence {
    type Name = rerun::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "user.Confidence".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        rerun::Float32::arrow_datatype()
    }

    #[inline]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        rerun::Float32::to_arrow_opt(data.into_iter().map(|opt| opt.map(Into::into).map(|c| c.0)))
    }
}

// ---

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_custom_data").spawn()?;

    rec.log(
        "left/my_confident_point_cloud",
        &CustomPoints3D {
            points3d: rerun::Points3D::new(grid(
                glam::Vec3::splat(-5.0),
                glam::Vec3::splat(5.0),
                3,
            )),
            confidences: Some(vec![42f32.into()]),
        },
    )?;

    rec.log(
        "right/my_polarized_point_cloud",
        &CustomPoints3D {
            points3d: rerun::Points3D::new(grid(
                glam::Vec3::splat(-5.0),
                glam::Vec3::splat(5.0),
                3,
            )),
            confidences: Some((0..27).map(|i| i as f32).map(Into::into).collect()),
        },
    )?;

    Ok(())
}
