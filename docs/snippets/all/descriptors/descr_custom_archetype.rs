use rerun::{
    external::arrow2, ChunkStore, ChunkStoreConfig, Component, ComponentDescriptor, VersionPolicy,
};

#[derive(Debug, Clone, Copy)]
struct CustomPosition3D(rerun::components::Position3D);

impl rerun::SizeBytes for CustomPosition3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl rerun::Loggable for CustomPosition3D {
    #[inline]
    fn arrow2_datatype() -> arrow2::datatypes::DataType {
        rerun::components::Position3D::arrow2_datatype()
    }

    #[inline]
    fn to_arrow2_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> rerun::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: 'a,
    {
        rerun::components::Position3D::to_arrow2_opt(
            data.into_iter().map(|opt| opt.map(Into::into).map(|c| c.0)),
        )
    }
}

impl rerun::Component for CustomPosition3D {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("user.CustomPosition3D")
    }
}

struct CustomPoints3D {
    positions: Vec<CustomPosition3D>,
    colors: Option<Vec<rerun::components::Color>>,
}

impl CustomPoints3D {
    fn indicator() -> rerun::NamedIndicatorComponent {
        rerun::NamedIndicatorComponent("user.CustomPoints3DIndicator".into())
    }

    fn overridden_position_descriptor() -> ComponentDescriptor {
        CustomPosition3D::descriptor()
            .or_with_archetype_name(|| "user.CustomPoints3D".into())
            .or_with_archetype_field_name(|| "custom_positions".into())
    }

    fn overridden_color_descriptor() -> ComponentDescriptor {
        rerun::components::Color::descriptor()
            .or_with_archetype_name(|| "user.CustomPoints3D".into())
            .or_with_archetype_field_name(|| "colors".into())
    }
}

impl rerun::AsComponents for CustomPoints3D {
    fn as_component_batches(&self) -> Vec<rerun::ComponentBatchCowWithDescriptor<'_>> {
        [
            Some(Self::indicator().to_batch()),
            Some(
                rerun::ComponentBatchCowWithDescriptor::new(&self.positions as &dyn rerun::ComponentBatch)
                    .with_descriptor_override(Self::overridden_position_descriptor()),
            ),
            self.colors.as_ref().map(|colors| {
                rerun::ComponentBatchCowWithDescriptor::new(colors as &dyn rerun::ComponentBatch)
                    .with_descriptor_override(Self::overridden_color_descriptor())
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[allow(clippy::unwrap_used)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    const APP_ID: &str = "rerun_example_descriptors_custom_archetype";

    let rec = rerun::RecordingStreamBuilder::new(APP_ID).spawn()?;

    let position = CustomPosition3D(rerun::components::Position3D::new(1.0, 2.0, 3.0));
    let color = rerun::components::Color::new(0xFF00FFFF);

    let points = CustomPoints3D {
        positions: vec![position],
        colors: Some(vec![color]),
    };

    rec.log_static("data", &points as _)?;

    // When this snippet runs through the snippet comparison machinery, this environment variable
    // will point to the output RRD.
    // We can thus load this RRD to check that the proper tags were indeed forwarded.
    //
    // Python and C++ are indirectly checked by the snippet comparison tool itself.
    if let Ok(path_to_rrd) = std::env::var("_RERUN_TEST_FORCE_SAVE") {
        rec.flush_blocking();

        let stores = ChunkStore::from_rrd_filepath(
            &ChunkStoreConfig::ALL_DISABLED,
            path_to_rrd,
            VersionPolicy::Warn,
        )?;
        assert_eq!(1, stores.len());

        let store = stores.into_values().next().unwrap();
        let chunks = store.iter_chunks().collect::<Vec<_>>();
        assert_eq!(1, chunks.len());

        let chunk = chunks.into_iter().next().unwrap();

        let mut descriptors = chunk
            .components()
            .values()
            .flat_map(|per_desc| per_desc.keys())
            .cloned()
            .collect::<Vec<_>>();
        descriptors.sort();

        let expected = vec![
            ComponentDescriptor {
                archetype_name: None,
                archetype_field_name: None,
                component_name: "user.CustomPoints3DIndicator".into(),
            },
            ComponentDescriptor {
                archetype_name: Some("user.CustomPoints3D".into()),
                archetype_field_name: Some("colors".into()),
                component_name: rerun::components::Color::name(),
            },
            ComponentDescriptor {
                archetype_name: Some("user.CustomPoints3D".into()),
                archetype_field_name: Some("custom_positions".into()),
                component_name: "user.CustomPosition3D".into(),
            },
        ];

        similar_asserts::assert_eq!(expected, descriptors);
    }

    Ok(())
}
