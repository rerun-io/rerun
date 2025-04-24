use rerun::{ChunkStore, ChunkStoreConfig, ComponentBatch as _, ComponentDescriptor};

fn example(rec: &rerun::RecordingStream) -> Result<(), Box<dyn std::error::Error>> {
    let positions = rerun::components::Position3D::new(1.0, 2.0, 3.0)
        .try_serialized()?
        .with_descriptor_override(ComponentDescriptor {
            archetype_name: Some("user.CustomArchetype".into()),
            archetype_field_name: Some("custom_positions".into()),
            component_name: "user.CustomPosition3D".into(),
        });
    rec.log_serialized_batches("data", true, [positions])?;

    Ok(())
}

// ---
// Everything below this line is _not_ part of the example.
// This is internal testing code to make sure the example yields the right data.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const APP_ID: &str = "rerun_example_descriptors_custom_component";
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

        let stores =
            ChunkStore::from_rrd_filepath(&ChunkStoreConfig::ALL_DISABLED, path_to_rrd).unwrap();
        assert_eq!(1, stores.len());

        let store = stores.into_values().next().unwrap();
        // Skip the first two chunks, as they represent the `RecordingProperties`.
        let chunks = store.iter_chunks().skip(2).collect::<Vec<_>>();
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
                archetype_name: Some("user.CustomArchetype".into()),
                archetype_field_name: Some("custom_positions".into()),
                component_name: "user.CustomPosition3D".into(),
            }, //
        ];

        similar_asserts::assert_eq!(expected, descriptors);
    }
}
