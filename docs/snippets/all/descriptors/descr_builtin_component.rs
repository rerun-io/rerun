use rerun::{ChunkStore, ChunkStoreConfig, ComponentDescriptor};

fn example(rec: &rerun::RecordingStream) -> Result<(), Box<dyn std::error::Error>> {
    use rerun::ComponentBatch as _;
    rec.log_static(
        "data",
        &[
            rerun::components::Position3D::new(1.0, 2.0, 3.0).try_serialized(
                ComponentDescriptor {
                    archetype: Some("user.CustomPoints3D".into()),
                    component: "user.CustomPoints3D:points".into(),
                    component_type: Some(
                        <rerun::components::Position3D as rerun::Component>::name(),
                    ),
                },
            )?,
        ],
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
        // Skip the first chunk, as it represents the `RecordingInfo`.
        let chunks = store.iter_physical_chunks().skip(1).collect::<Vec<_>>();
        assert_eq!(1, chunks.len());

        let chunk = chunks.into_iter().next().unwrap();

        let mut descriptors = chunk
            .components()
            .component_descriptors()
            .cloned()
            .collect::<Vec<_>>();
        descriptors.sort();

        let expected = vec![
            ComponentDescriptor {
                archetype: Some("user.CustomPoints3D".into()),
                component: "user.CustomPoints3D:points".into(),
                component_type: Some(<rerun::components::Position3D as rerun::Component>::name()),
            }, //
        ];

        similar_asserts::assert_eq!(expected, descriptors);
    }
}
