use rerun::{ChunkStore, ChunkStoreConfig, Component as _, ComponentDescriptor, VersionPolicy};

fn example(rec: &rerun::RecordingStream) -> Result<(), Box<dyn std::error::Error>> {
    use rerun::ComponentBatch as _;
    rec.log_static(
        "data",
        &[rerun::components::Position3D::new(1.0, 2.0, 3.0).try_serialized()?],
    )?;

    Ok(())
}

// ---
// Everything below this line is _not_ part of the example.
// This is internal testing code to make sure the example yields the right data.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const APP_ID: &str = "rerun_example_descriptors_builtin_component";
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
        assert_eq!(3, chunks.len());

        {
            let chunk = &chunks[0];

            let mut descriptors = chunk
                .components()
                .values()
                .flat_map(|per_desc| per_desc.keys())
                .cloned()
                .collect::<Vec<_>>();
            descriptors.sort();

            let expected = vec![ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.RecordingProperties".into()),
                archetype_field_name: Some("start_time".into()),
                component_name: "rerun.components.Timestamp".into(),
            }];

            similar_asserts::assert_eq!(expected, descriptors);
        }

        {
            let chunk = &chunks[1];

            let mut descriptors = chunk
                .components()
                .values()
                .flat_map(|per_desc| per_desc.keys())
                .cloned()
                .collect::<Vec<_>>();
            descriptors.sort();

            let expected = vec![ComponentDescriptor {
                archetype_name: None,
                archetype_field_name: None,
                component_name: "rerun.components.RecordingPropertiesIndicator".into(),
            }];

            similar_asserts::assert_eq!(expected, descriptors);
        }

        {
            let chunk = &chunks[2];

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
                    component_name: rerun::components::Position3D::name(),
                }, //
            ];

            similar_asserts::assert_eq!(expected, descriptors);
        }
    }
}
