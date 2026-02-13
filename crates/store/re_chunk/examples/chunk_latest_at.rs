use re_chunk::{Chunk, LatestAtQuery, RowId, Timeline, TimelineName};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};

// ---

fn main() -> anyhow::Result<()> {
    let chunk = create_chunk()?;

    eprintln!("Data:\n{chunk}");

    let query = LatestAtQuery::new(TimelineName::new("frame"), 4);

    // Find all relevant data for a query:
    let chunk = chunk.latest_at(&query, MyPoints::descriptor_points().component);
    eprintln!("{:?} @ {query:?}:\n{chunk}", MyPoints::descriptor_points());

    // And then slice it as appropriate:
    let chunk = chunk
        .timeline_sliced(TimelineName::log_time())
        .component_sliced(MyPoints::descriptor_points().component);
    eprintln!("Sliced down to specific timeline and component:\n{chunk}");

    Ok(())
}

fn create_chunk() -> anyhow::Result<Chunk> {
    let mut chunk = Chunk::builder("my/entity")
        .with_component_batches(
            RowId::new(),
            [
                (Timeline::log_time(), 1000),
                (Timeline::new_sequence("frame"), 1),
            ],
            [(
                MyPoints::descriptor_points(),
                &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)] as _,
            )],
        )
        .with_component_batches(
            RowId::new(),
            [
                (Timeline::log_time(), 1032),
                (Timeline::new_sequence("frame"), 3),
            ],
            [
                (
                    MyPoints::descriptor_colors(),
                    &[MyColor::from_rgb(1, 1, 1)] as _,
                ),
                (
                    MyPoints::descriptor_labels(),
                    &[
                        MyLabel("a".into()),
                        MyLabel("b".into()),
                        MyLabel("c".into()),
                    ] as _,
                ),
            ],
        )
        .with_component_batches(
            RowId::new(),
            [
                (Timeline::log_time(), 1064),
                (Timeline::new_sequence("frame"), 5),
            ],
            [(
                MyPoints::descriptor_points(),
                &[
                    MyPoint::new(3.0, 3.0),
                    MyPoint::new(4.0, 4.0),
                    MyPoint::new(5.0, 5.0),
                ] as _,
            )],
        )
        .build()?;

    chunk.sort_if_unsorted();

    Ok(chunk)
}
