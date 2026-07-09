//! Use a blueprint to show a tensor view.

use ndarray::Array;
use rerun::{
    blueprint::{
        Blueprint, TensorView, archetypes as blueprint_archetypes, components,
    },
    components::{Colormap, MagnificationFilter},
    datatypes::TensorDimensionSelection,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        TensorView::new("Tensor")
            .with_origin("tensor")
            .with_slice_selection(
                &blueprint_archetypes::TensorSliceSelection::new()
                    .with_width(1)
                    .with_height(TensorDimensionSelection {
                        dimension: 2,
                        invert: true,
                    })
                    .with_indices([
                        rerun::components::TensorDimensionIndexSelection::new(
                            2, 4,
                        ),
                        rerun::components::TensorDimensionIndexSelection::new(
                            3, 5,
                        ),
                    ])
                    .with_slider([2]),
            )
            .with_scalar_mapping(
                &blueprint_archetypes::TensorScalarMapping::new()
                    .with_colormap(Colormap::Turbo)
                    .with_gamma(1.5)
                    .with_mag_filter(MagnificationFilter::Linear),
            )
            .with_view_fit(
                &blueprint_archetypes::TensorViewFit::new()
                    .with_scaling(components::ViewFit::Fill),
            ),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_tensor")
        .with_blueprint(blueprint)
        .spawn()?;

    let tensor =
        Array::from_shape_fn((32, 240, 320, 3), |(batch, x, y, channel)| {
            (batch * 17 + x * 5 + y * 3 + channel * 97) as u8
        });
    rec.log(
        "tensor",
        &rerun::Tensor::try_from(tensor)?
            .with_dim_names(["batch", "x", "y", "channel"]),
    )?;

    Ok(())
}
