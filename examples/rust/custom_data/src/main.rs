//! Demonstrates how to implement custom archetypes and components, and extend existing ones.

use rerun::{
    archetypes::Points3D,
    datatypes::Float32,
    demo_util::grid,
    external::{arrow2, glam, re_types},
    AnyComponentList, Archetype, ArchetypeName, ComponentList, ComponentName,
    GenericIndicatorComponent, Loggable, RecordingStreamBuilder,
};

// ---

type CustomPoints3DIndicator = GenericIndicatorComponent<CustomPoints3D>;

/// A custom [`Archetype`] that extends Rerun's builtin [`Points3D`] archetype with extra
/// [`rerun::Component`]s.
struct CustomPoints3D {
    points3d: Points3D,
    confidences: Option<Vec<Confidence>>,
}

impl Archetype for CustomPoints3D {
    type Indicator = CustomPoints3DIndicator;

    fn name() -> ArchetypeName {
        "user.CustomPoints3D".into()
    }

    fn required_components() -> std::borrow::Cow<'static, [rerun::ComponentName]> {
        Points3D::required_components()
    }

    fn recommended_components() -> std::borrow::Cow<'static, [rerun::ComponentName]> {
        Points3D::recommended_components()
            .iter()
            .copied()
            .chain([Confidence::name()])
            .collect::<Vec<_>>()
            .into()
    }

    fn optional_components() -> std::borrow::Cow<'static, [rerun::ComponentName]> {
        Points3D::optional_components()
    }

    fn num_instances(&self) -> usize {
        self.points3d.num_instances()
    }

    fn as_component_lists(&self) -> Vec<AnyComponentList<'_>> {
        self.points3d
            .as_component_lists()
            .into_iter()
            .chain(
                [
                    Some(Self::Indicator::new_list(self.num_instances()).into()),
                    self.confidences
                        .as_ref()
                        .map(|v| (v as &dyn ComponentList).into()),
                ]
                .into_iter()
                .flatten(),
            )
            .collect()
    }
}

// ---

/// A custom [`rerun::Component`] that is backed by a builtin [`Float32`] scalar [`rerun::Datatype`].
#[derive(Debug, Clone, Copy)]
struct Confidence(Float32);

impl From<f32> for Confidence {
    fn from(v: f32) -> Self {
        Self(Float32(v))
    }
}

impl Loggable for Confidence {
    type Name = ComponentName;

    fn name() -> Self::Name {
        "user.Confidence".into()
    }

    fn arrow_datatype() -> arrow2::datatypes::DataType {
        Float32::arrow_datatype()
    }

    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> re_types::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        Float32::try_to_arrow_opt(data.into_iter().map(|opt| opt.map(Into::into).map(|c| c.0)))
    }
}

// ---

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_custom_data").memory()?;

    rec.log(
        "left/my_confident_point_cloud",
        &CustomPoints3D {
            points3d: Points3D::new(grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10)),
            confidences: Some(vec![42f32.into()]),
        },
    )?;

    rec.log(
        "right/my_polarized_point_cloud",
        &CustomPoints3D {
            points3d: Points3D::new(grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10)),
            confidences: Some((0..1000).map(|i| i as f32).map(Into::into).collect()),
        },
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
