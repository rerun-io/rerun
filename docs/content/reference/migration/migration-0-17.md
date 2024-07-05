---
title: Migrating from 0.16 to 0.17
order: 170
---


## New and changed components

* New [`ImagePlaneDistance`](https://rerun.io/docs/reference/types/components/image_plane_distance) to allow configuring the size of the Pinhole frustum visualization.
* New [`AxisLength`](https://rerun.io/docs/reference/types/components/axis_length) to allow configuring the axis length of the transform visualization.
* New components for the `DepthImage` and `SegmentationImage` archetypes:
    * [`Opacity`](https://rerun.io/docs/reference/types/components/opacity) is used to configure transparency.
        * Note: layered `Image` are no longer made automatically transparent
    * [`FillRatio`](https://rerun.io/docs/reference/types/components/fill_ratio) is used for setting the point radius on `DepthImage` in 3D views.
    * [`Colormap`](https://rerun.io/docs/reference/types/components/colormap) is used for setting `DepthImage` colormap.
    * [`AggregationPolicy`](https://rerun.io/docs/reference/types/components/aggregation_policy) is used for setting aggregation policy on line plots.
* [`Radius`](https://rerun.io/docs/reference/types/components/radius) component can now optionally specify radius in UI points
* Renamed [`HalfSize2D`](https://rerun.io/docs/reference/types/components/half_size2d) and [`HalfSize3D`](https://rerun.io/docs/reference/types/components/half_size3d). They were previously in plural form. All our components now are in singular form.