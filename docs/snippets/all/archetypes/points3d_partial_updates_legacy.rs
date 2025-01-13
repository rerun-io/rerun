//! Demonstrates usage of the legacy partial updates APIs.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points3d_partial_updates_legacy")
        .spawn()?;

    let positions = (0..10).map(|i| (i as f32, 0.0, 0.0));

    rec.set_time_sequence("frame", 0);
    rec.log("points", &rerun::Points3D::new(positions))?;

    for i in 0..10 {
        let colors: Vec<rerun::components::Color> = (0..10)
            .map(|n| {
                if n < i {
                    rerun::Color::from_rgb(20, 200, 20)
                } else {
                    rerun::Color::from_rgb(200, 20, 20)
                }
            })
            .collect();
        let radii: Vec<rerun::components::Radius> = (0..10)
            .map(|n| if n < i { 0.6 } else { 0.2 })
            .map(Into::into)
            .collect();

        rec.set_time_sequence("frame", i);
        rec.log("points", &[&colors as &dyn rerun::ComponentBatch, &radii])?;
    }

    use rerun::Archetype as _;
    let positions: Vec<rerun::components::Position3D> =
        (0..10).map(|i| (i as f32, 0.0, 0.0).into()).collect();

    rec.set_time_sequence("frame", 20);
    rec.log(
        "points",
        &[
            &rerun::Points3D::indicator() as &dyn rerun::ComponentBatch,
            &positions,
            &vec![rerun::components::Radius(0.3.into())],
            &Vec::<rerun::components::Color>::new(),
            &Vec::<rerun::components::Text>::new(),
            &Vec::<rerun::components::ShowLabels>::new(),
            &Vec::<rerun::components::ClassId>::new(),
            &Vec::<rerun::components::KeypointId>::new(),
        ],
    )?;

    Ok(())
}
