---
title: Visualize geospatial data
order: 300
---

Rerun 0.20 introduced a new [map view](../../reference/types/views/map_view.md).
This guide provides a short overview on how to use it to visualise geospatial data.

## Coordinate system

The map view uses the [ESPG:3857](https://epsg.io/3857) [spherical mercator projection](https://en.wikipedia.org/wiki/Web_Mercator_projection) commonly used by web services such as [OpenStreetMap](https://www.openstreetmap.org/).
This enables the use of commonly available web tiles for the background map.

To be compatible with this view, geospatial data must be expressed using [ESPG:4326](https://epsg.io/4326) (aka WGS84) latitudes and longitudes.
This corresponds to what is commonly referred to as "GPS coordinates."
Rerun provides a set of archetypes prefixed with `Geo` designed to encapsulate such data.

For example, [`GeoPoints`](../../reference/types/archetypes/geo_points.md) represent a single geospatial location (or a batch thereof). The location of the Eiffel Tower can be logged as follows:

```python
rr.log("eiffel_tower", rr.GeoPoints(lat_lon=[48.858222, 2.2945]))
```

Both the latitude and longitude must be provided in degrees, with positive values corresponding to the North, resp. East directions.

Note that Rerun always expects latitudes first and longitudes second.
As there is [no accepted ordering standard](https://stackoverflow.com/questions/7309121/preferred-order-of-writing-latitude-longitude-tuples-in-gis-services), our APIs strive to make this ordering choice as explicit as possible.
In this case, the `lat_lon` argument is keyword-only and must thus be explicitly named as a reminder of this order.


## Types of geometries

Rerun currently supports two types of geometries:

- [`GeoPoints`](../../reference/types/archetypes/geo_points.md): batch of individual points, with optional [radius](../../reference/types/components/radius.md) and [color](../../reference/types/components/color.md)
- [`GeoLineStrings`](../../reference/types/archetypes/geo_line_strings.md): batch of line strings, with optional [radius](../../reference/types/components/radius.md) and [color](../../reference/types/components/color.md)

*Note*: polygons are planned but are not supported yet (see [this issue](https://github.com/rerun-io/rerun/issues/8066)).

As in other views, radii may be expressed either as UI points (negative values) or scene units (positive values).
For the latter case, the map view uses meters are scene units.

Apart from the use of latitude and longitude, `GeoPoints` and `GeoLineStrings` are otherwise similar to the [`Points2D`](../../reference/types/archetypes/points2d.md) and [`LineStrip2D`](../../reference/types/archetypes/line_strips2d.md) archetypes used in the [2D view](../../reference/types/views/spatial2d_view.md).


## Using Mapbox background maps <!-- NOLINT -->

The map view supports several types of background maps, including a few from [Mapbox](https://www.mapbox.com).
A Mapbox access token is required to use them.
It must be provided either using the `RERUN_MAPBOX_ACCESS_TOKEN` environment variable or configured in the settings screen ("Settings…" item in the Rerun menu).
An access token may be freely obtained by creating a Mapbox account.


## Creating a map view from code

Like other views, the map view can be configured using the [blueprint API](../../getting-started/configure-the-viewer.md#programmatic-blueprints):

```python
import rerun.blueprint as rrb

blueprint = rrb.Blueprint(
    rrb.MapView(
        origin="/robot/position",
        name="map view",
        zoom=16.0,
        background=rrb.MapProvider.OpenStreetMap,
    ),
)
```

Check the [map view](../../reference/types/views/map_view.md) reference for details.
