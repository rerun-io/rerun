---
title: Tensor
order: 60
description: Inspect slices of multidimensional arrays
---

Use a tensor view to inspect multidimensional arrays as color-mapped slices.
Choose which dimensions to display and step through the others to explore feature maps, activation tensors, or other dense numeric data.

See the [`TensorView` reference](../../reference/types/views/tensor_view.md) for properties and examples.

## Data for this view

Log a [`Tensor`](../../reference/types/archetypes/tensor.md) to inspect its multidimensional values.

## Configure the view

### Select a slice and its appearance

Use `slice_selection` to choose the displayed dimensions and the indices of the remaining dimensions.
Use `scalar_mapping` for the colormap, gamma, and magnification filter, and `view_fit` for sizing.
If `slider` is omitted, every dimension in `indices` gets a slider.
See the [TensorView reference](../../reference/types/views/tensor_view.md) for an example and [Configure a recording layout from code](../inspect-recordings/configure-from-code.md) for the blueprint workflow.

## Related

See [Dataframe](dataframe.md) for values in a table and [Raw data](raw-data.md) for component inspection.
