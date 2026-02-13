use rerun::{ChunkStore, ChunkStoreConfig, ComponentDescriptor};

fn example(rec: &rerun::RecordingStream) -> Result<(), Box<dyn std::error::Error>> {
    rec.log_static(
        "data",
        &rerun::Points3D::new([(1.0, 2.0, 3.0)]).with_radii([0.3, 0.2, 0.1]),
    )?;

    Ok(())
}

// ---
// Everything below this line is _not_ part of the example.
// This is internal testing code to make sure the example yields the right data.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const APP_ID: &str = "rerun_example_descriptors_builtin_archetype";
    let rec = rerun::RecordingStreamBuilder::new(APP_ID).spawn()?;

    example(&rec)?;

    check_tags(&rec);

    Ok(())
}

#[expect(clippy::unwrap_used)]
fn check_tags(rec: &rerun::RecordingStream) {
    // When this snippet runs through the snippet comparison machinery, this environment variable
    // will point to the output RRD.
    // We can thus load this RRD to check that the proper tags were indeed forwarded.
    //
    // Python and C++ are indirectly checked by the snippet comparison tool itself.
    if let Ok(path_to_rrd) = std::env::var("_RERUN_TEST_FORCE_SAVE") {
        rec.flush_blocking().unwrap();

        let stores =
            ChunkStore::from_rrd_filepath(&ChunkStoreConfig::ALL_DISABLED, path_to_rrd).unwrap();
        assert_eq!(1, stores.len());

        let store = stores.into_values().next().unwrap();
        // Skip the first chunk, as it represent the `RecordingInfo`.
        let chunks = store.iter_physical_chunks().skip(1).collect::<Vec<_>>();
        assert_eq!(1, chunks.len());

        {
            let chunk = &chunks[0];

            let mut descriptors = chunk
                .components()
                .component_descriptors()
                .cloned()
                .collect::<Vec<_>>();
            descriptors.sort();

            let expected = vec![
                ComponentDescriptor {
                    archetype: Some("rerun.archetypes.Points3D".into()),
                    component: "Points3D:positions".into(),
                    component_type: Some("rerun.components.Position3D".into()),
                },
                ComponentDescriptor {
                    archetype: Some("rerun.archetypes.Points3D".into()),
                    component: "Points3D:radii".into(),
                    component_type: Some("rerun.components.Radius".into()),
                },
            ];

            similar_asserts::assert_eq!(expected, descriptors);
        }
    }
}
