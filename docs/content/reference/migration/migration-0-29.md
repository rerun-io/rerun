---
title: Migrating from 0.28 to 0.29
order: 981
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Deprecated `rerun.dataframe` API has been removed

The `rerun.dataframe` module and its associated APIs, which were deprecated in 0.28, have now been fully removed. This includes `RecordingView`, `Recording.view()`, and the ability to run dataframe queries locally via this module.

Please refer to the [0.28 migration guide section on `RecordingView` and local dataframe API](migration-0-28.md#recordingview-and-local-dataframe-api-deprecated) for details on updating your code to use `rerun.server.Server` and the `rerun.catalog` API instead.

## Deprecated `rerun.catalog` APIs have been removed

The deprecated `rerun.catalog` APIs that were marked for removal in 0.28 have now been fully removed. If you were using any of these deprecated methods, you must update your code to use the new APIs.

Please refer to the [0.28 migration guide section on catalog API overhaul](migration-0-28.md#python-sdk-catalog-api-overhaul) for more details on the new API patterns.

## New/changed API for defining value & visualizer overrides

The Python API for setting blueprint overrides now uses special visualizer objects:

Before:
```py
rr.send_blueprint(
    rrb.Spatial2DView(
        overrides={"boxes/1": rr.Boxes2D.from_fields(colors=[0, 255, 0])},
    ),
)
```
After:
```py
rr.send_blueprint(
    rrb.Spatial2DView(
        visualizer_overrides={"boxes/1": rrb.visualizers.Boxes2D(colors=[0, 255, 0])},
    ),
)
```

<!-- TODO(RR-3254): While we're here, might as well mention what this looks like with mappings! -->

The same API is also used to set which visualizers should be used in the first place (instead of relying on a automatic, archetype-based selection):

Before:
```py
rr.send_blueprint(
    rrb.TimeSeriesView(
        overrides={
            "trig/sin": [
                rrb.VisualizerOverrides([rrb.visualizers.SeriesLines, rrb.visualizers.SeriesPoints]),
            ],
        },
    )
)
```
After:
```py
rr.send_blueprint(
    rrb.TimeSeriesView(
        visualizer_overrides={
            "trig/sin": [
                rrb.visualizers.SeriesLines(),
                rrb.visualizers.SeriesPoints(),
            ],
        },
    )
)
```

<!-- TODO(RR-3153): While we're here, illustrate the other motivation a bit. Something like:
Note that this now allows to specify overrides for multiple instances of the same visualizer: EXAMPLE HERE. -->
