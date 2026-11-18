---
title: Tensor
order: 60
description: Inspect slices of multidimensional arrays
---

Use a tensor view to inspect multidimensional arrays as color-mapped slices.
Choose which dimensions to display and step through the others to explore feature maps, activation tensors, or other dense numeric data.

See the [`TensorView` reference](../../reference/types/views/tensor_view.md) for properties and examples.

## Select a slice and its appearance

Use `slice_selection` to choose the displayed dimensions and the indices of the remaining dimensions.
Use `scalar_mapping` for the colormap, gamma, and magnification filter, and `view_fit` for sizing.
If `slider` is omitted, every dimension in `indices` gets a slider.
This example assumes a tensor with shape `(32, 240, 320, 3)`.

```python
import rerun as rr
import rerun.blueprint as rrb

rrb.TensorView(
    origin="tensor",
    # Explicitly pick which dimensions to show.
    slice_selection=rrb.TensorSliceSelection(
        # Width and height select axes; indices select slices on the remaining axes.
        width=1,
        height=rr.TensorDimensionSelection(dimension=2, invert=True),
        # Set which indices to show for the other dimensions.
        indices=[
            rr.TensorDimensionIndexSelection(dimension=0, index=4),
            rr.TensorDimensionIndexSelection(dimension=3, index=2),
        ],
        # Dimensions omitted from `slider` keep a fixed index.
        slider=[0],
    ),
    # Set a scalar mapping with a custom colormap, gamma and magnification filter.
    scalar_mapping=rrb.TensorScalarMapping(colormap="turbo", gamma=1.5, mag_filter="linear"),
    # Fill the view, ignoring aspect ratio.
    view_fit="fill",
)
```
