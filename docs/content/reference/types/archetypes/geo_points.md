---
title: "GeoPoints"
---
<!-- DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/docs/website.rs -->

Geospatial points with positions expressed in [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees), and optional colors and radii.

## Fields
### Required
* `positions`: [`LatLon`](../components/lat_lon.md)

### Recommended
* `radii`: [`Radius`](../components/radius.md)
* `colors`: [`Color`](../components/color.md)

### Optional
* `class_ids`: [`ClassId`](../components/class_id.md)


## Can be shown in
* [MapView](../views/map_view.md)
* [DataframeView](../views/dataframe_view.md)

## API reference links
 * 🌊 [C++ API docs for `GeoPoints`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1GeoPoints.html)
 * 🐍 [Python API docs for `GeoPoints`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.GeoPoints)
 * 🦀 [Rust API docs for `GeoPoints`](https://docs.rs/rerun/latest/rerun/archetypes/struct.GeoPoints.html)

## Example

### Log a geospatial point

snippet: archetypes/geo_points_simple

<picture data-inline-viewer="snippets/geo_points_simple">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/1200w.png">
  <img src="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/full.png">
</picture>

