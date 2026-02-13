//! Demonstrates how to implement custom archetypes and components, and extend existing ones.

use rerun::{
    ComponentBatch as _, ComponentDescriptor, SerializedComponentBatch,
    demo_util::grid,
    external::{arrow, glam, re_sdk_types},
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
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        self.points3d
            .as_serialized_batches()
            .into_iter()
            .chain(
                std::iter::once(self.confidences.as_ref().and_then(|batch| {
                    batch.serialized(ComponentDescriptor {
                        archetype: Some("user.CustomPoints3D".into()),
                        component: "user.CustomPoints3D:confidences".into(),
                        component_type: Some(<Confidence as rerun::Component>::name()),
                    })
                }))
                .flatten(),
            )
            .collect()
    }
}

// ---

/// A custom [`rerun::Component`] that is backed by a builtin [`rerun::Float32`] scalar.
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
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        rerun::Float32::arrow_datatype()
    }

    #[inline]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_sdk_types::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        rerun::Float32::to_arrow_opt(data.into_iter().map(|opt| opt.map(Into::into).map(|c| c.0)))
    }
}

impl rerun::Component for Confidence {
    #[inline]
    fn name() -> rerun::ComponentType {
        "user.Confidence".into()
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
