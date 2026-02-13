---
title: Migrating from 0.16 to 0.17
order: 993
---


## ⚠️ Breaking changes
* `HalfSizes2D` has been renamed to [`HalfSize2D`](https://rerun.io/docs/reference/types/components/half_size2d)
* `HalfSizes3D` has been renamed to [`HalfSize3D`](https://rerun.io/docs/reference/types/components/half_size3d)
* `.rrd` files from older versions won't load in Rerun 0.17


## New integrated visualizer and component override UI

The visualizer and component override UI of the timeseries views has been unified and overhauled. It is also now used for all view kinds (it was previously only available for timeseries views).

In 0.16.1 and earlier:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/vizcomp-ui-before/ec7c0b88cdb54420665de32aaf2096dfd3dc05ea/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/vizcomp-ui-before/ec7c0b88cdb54420665de32aaf2096dfd3dc05ea/480w.png">
</picture>

In 0.17.0:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/vizcomp-ui-after/86f74d239e8b77bc3df00e61cfc35eb9f4c07989/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/vizcomp-ui-after/86f74d239e8b77bc3df00e61cfc35eb9f4c07989/480w.png">
</picture>

See [Visualizers and Overrides](../../concepts/visualization/visualizers-and-overrides.md) for more information.


## New blueprint API to specify component overrides, visualizer overrides, and component defaults

This release introduces new Python APIs to set component overrides, visualizer overrides, and component defaults from code. Depending on your use-case, these new APIs become the preferred way of styling views.

For example, setting color and enabling the `SeriesPoints` visualizer was previously done using `rr.log()`:

```python
rr.log("data", rr.SeriesPoints(colors=[255, 255, 0]), static=True)

for t in range(1000):
    rr.set_time_sequence("frame_nr", t)
    rr.log("data",rr.Scalar(get_data(t))),

rr.send_blueprint(
    rr.blueprint.TimeSeriesView(origin="data")
)
```

Now the override can be specified from the blueprint, removing the need to include styling information in the data store:

```python
for t in range(1000):
    rr.set_time_sequence("frame_nr", t)
    rr.log("data",rr.Scalar(get_data(t))),

rr.send_blueprint(
    rr.blueprint.TimeSeriesView(
        origin="data",
        overrides={
            "data": [
                rr.blueprint.VisualizerOverrides("SeriesPoints"),
                rr.components.Color([255, 255, 0])
            ]
        },
    )
)
```

The [Plots](https://rerun.io/examples/feature-showcase/plots) example has been updated to showcase the new APIs. See [Visualizers and Overrides](../../concepts/visualization/visualizers-and-overrides.md) for more information.


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
