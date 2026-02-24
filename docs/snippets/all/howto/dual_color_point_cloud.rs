//! Demonstrates how to visualize the same point cloud with two different color schemes.
//!
//! Two custom archetypes (using Rerun's Color component type) are logged on the same entity,
//! then a blueprint maps each color set to a separate 3D view.

use std::f64::consts::TAU;

use rand::prelude::*;

use rerun::blueprint::VisualizableArchetype as _;

fn colormap(t: f64, stops: &[(f64, [u8; 3])]) -> rerun::components::Color {
    for i in 0..stops.len() - 1 {
        if t <= stops[i + 1].0 {
            let frac = ((t - stops[i].0) / (stops[i + 1].0 - stops[i].0)) as f32;
            let [r0, g0, b0] = stops[i].1.map(|c| c as f32);
            let [r1, g1, b1] = stops[i + 1].1.map(|c| c as f32);
            return rerun::components::Color::from_rgb(
                (r0 + frac * (r1 - r0)) as u8,
                (g0 + frac * (g1 - g0)) as u8,
                (b0 + frac * (b1 - b0)) as u8,
            );
        }
    }
    let [r, g, b] = stops.last().expect("stops must not be empty").1;
    rerun::components::Color::from_rgb(r, g, b)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_custom_color_archetypes").spawn()?;

    // --- Generate a torus point cloud ---
    let n = 8_000;
    let mut rng = SmallRng::seed_from_u64(42);

    let theta: Vec<f64> = (0..n).map(|_| rng.random_range(0.0..TAU)).collect(); // angle around ring
    let phi: Vec<f64> = (0..n).map(|_| rng.random_range(0.0..TAU)).collect(); // angle around tube

    let positions: Vec<[f32; 3]> = theta
        .iter()
        .zip(&phi)
        .map(|(&t, &p)| {
            let r = 3.0 + p.cos();
            [(r * t.cos()) as f32, (r * t.sin()) as f32, p.sin() as f32]
        })
        .collect();

    // --- Color scheme 1: height (z-coordinate), cool-to-warm ---
    let cool_warm = [
        (0.0, [59, 76, 192]),
        (0.5, [220, 220, 220]),
        (1.0, [180, 4, 38]),
    ];
    let height_colors: Vec<_> = phi
        .iter()
        .map(|&p| colormap(f64::midpoint(p.sin(), 1.0), &cool_warm))
        .collect();

    // --- Color scheme 2: toroidal angle, cyclic (teal -> purple -> orange -> teal) ---
    let cyclic = [
        (0.0, [0, 200, 200]),
        (0.25, [120, 40, 200]),
        (0.5, [255, 140, 50]),
        (0.75, [200, 220, 60]),
        (1.0, [0, 200, 200]),
    ];
    let spin_colors: Vec<_> = theta.iter().map(|&t| colormap(t / TAU, &cyclic)).collect();

    // region: log_custom_archetypes
    // --- Log positions and both color sets in one call ---
    rec.log(
        "pointcloud",
        &[
            &rerun::Points3D::new(positions).with_radii([rerun::components::Radius::from(0.06)])
                as &dyn rerun::AsComponents,
            &rerun::DynamicArchetype::new("HeightColors")
                .with_component::<rerun::components::Color>("colors", height_colors),
            &rerun::DynamicArchetype::new("SpinColors")
                .with_component::<rerun::components::Color>("colors", spin_colors),
        ],
    )?;
    // endregion: log_custom_archetypes

    // region: blueprint
    // --- Blueprint: two side-by-side 3D views with different color mappings ---
    let blueprint = rerun::blueprint::Blueprint::new(rerun::blueprint::Horizontal::new([
        rerun::blueprint::Spatial3DView::new("Height Colors")
            .with_origin("/")
            .with_overrides(
                "pointcloud",
                [rerun::Points3D::update_fields()
                    .visualizer()
                    .with_mappings(vec![
                        rerun::blueprint::VisualizerComponentMapping::new_source_component(
                            rerun::Points3D::descriptor_colors().component,
                            "HeightColors:colors",
                        )
                        .into(),
                    ])],
            )
            .into(),
        rerun::blueprint::Spatial3DView::new("Spin Colors")
            .with_origin("/")
            .with_overrides(
                "pointcloud",
                [rerun::Points3D::update_fields()
                    .visualizer()
                    .with_mappings(vec![
                        rerun::blueprint::VisualizerComponentMapping::new_source_component(
                            rerun::Points3D::descriptor_colors().component,
                            "SpinColors:colors",
                        )
                        .into(),
                    ])],
            )
            .into(),
    ]));

    blueprint.send(&rec, rerun::blueprint::BlueprintActivation::default())?;
    // endregion: blueprint

    Ok(())
}
