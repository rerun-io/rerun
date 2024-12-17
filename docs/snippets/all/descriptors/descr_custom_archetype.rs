use rerun::{ChunkStore, ChunkStoreConfig, ComponentBatch, ComponentDescriptor, VersionPolicy};

struct CustomPoints3D {
    positions: Vec<rerun::components::Position3D>,
    colors: Option<Vec<rerun::components::Color>>,
}

impl CustomPoints3D {
    fn indicator() -> rerun::NamedIndicatorComponent {
        rerun::NamedIndicatorComponent("user.CustomPoints3DIndicator".into())
    }

    fn overridden_position_descriptor() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("user.CustomPoints3D".into()),
            archetype_field_name: Some("custom_positions".into()),
            component_name: "user.CustomPosition3D".into(),
        }
    }

    fn overridden_color_descriptor() -> ComponentDescriptor {
        <rerun::components::Color as rerun::Component>::descriptor()
            .or_with_archetype_name(|| "user.CustomPoints3D".into())
            .or_with_archetype_field_name(|| "colors".into())
    }
}

impl rerun::AsComponents for CustomPoints3D {
    fn as_component_batches(&self) -> Vec<rerun::ComponentBatchCowWithDescriptor<'_>> {
        [
            Some(Self::indicator().to_batch()),
            Some(
                self.positions
                    .with_descriptor(Self::overridden_position_descriptor()),
            ),
            self.colors
                .as_ref()
                .map(|colors| colors.with_descriptor(Self::overridden_color_descriptor())),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

fn example(rec: &rerun::RecordingStream) -> Result<(), Box<dyn std::error::Error>> {
    let positions = rerun::components::Position3D::new(1.0, 2.0, 3.0);
    let colors = rerun::components::Color::new(0xFF00FFFF);

    let points = CustomPoints3D {
        positions: vec![positions],
        colors: Some(vec![colors]),
    };

    rec.log_static("data", &points as _)?;

    Ok(())
}

// ---
// Everything below this line is _not_ part of the example.
// This is internal testing code to make sure the example yields the right data.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const APP_ID: &str = "rerun_example_descriptors_custom_archetype";
    let rec = rerun::RecordingStreamBuilder::new(APP_ID).spawn()?;

    example(&rec)?;

    check_tags(&rec);

    Ok(())
}

#[allow(clippy::unwrap_used)]
fn check_tags(rec: &rerun::RecordingStream) {
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
        )
        .unwrap();
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

        // TODO(cmc): revert me
        // let expected = vec![
        //     ComponentDescriptor {
        //         archetype_name: None,
        //         archetype_field_name: None,
        //         component_name: "user.CustomPoints3DIndicator".into(),
        //     },
        //     ComponentDescriptor {
        //         archetype_name: Some("user.CustomPoints3D".into()),
        //         archetype_field_name: Some("colors".into()),
        //         component_name: "rerun.components.Color".into(),
        //     },
        //     ComponentDescriptor {
        //         archetype_name: Some("user.CustomPoints3D".into()),
        //         archetype_field_name: Some("custom_positions".into()),
        //         component_name: "user.CustomPosition3D".into(),
        //     },
        // ];
        let expected = vec![
            ComponentDescriptor {
                archetype_name: None,
                archetype_field_name: None,
                component_name: "rerun.components.Color".into(),
            },
            ComponentDescriptor {
                archetype_name: None,
                archetype_field_name: None,
                component_name: "user.CustomPoints3DIndicator".into(),
            },
            ComponentDescriptor {
                archetype_name: None,
                archetype_field_name: None,
                component_name: "user.CustomPosition3D".into(),
            },
        ];

        similar_asserts::assert_eq!(expected, descriptors);
    }
}
