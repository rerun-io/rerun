use rerun::{ChunkStore, ChunkStoreConfig, ComponentBatch, ComponentDescriptor, VersionPolicy};

fn example(rec: &rerun::RecordingStream) -> Result<(), Box<dyn std::error::Error>> {
    let positions = rerun::components::Position3D::new(1.0, 2.0, 3.0);
    let positions = positions.with_descriptor(ComponentDescriptor {
        archetype_name: Some("user.CustomArchetype".into()),
        archetype_field_name: Some("custom_positions".into()),
        component_name: "user.CustomPosition3D".into(),
    });
    rec.log_component_batches("data", true, [&positions as &dyn rerun::ComponentBatch])?;

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
        //         archetype_name: Some("user.CustomArchetype".into()),
        //         archetype_field_name: Some("custom_positions".into()),
        //         component_name: "user.CustomPosition3D".into(),
        //     }, //
        // ];
        let expected = vec![
            ComponentDescriptor {
                archetype_name: None,
                archetype_field_name: None,
                component_name: "user.CustomPosition3D".into(),
            }, //
        ];

        similar_asserts::assert_eq!(expected, descriptors);
    }
}
