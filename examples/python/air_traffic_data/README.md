<!--[metadata]
title = "Air traffic data"
tags = ["2D", "3D", "map", "crs"]
description = "Display aircraft traffic data"
thumbnail = "https://static.rerun.io/air_traffic_data/348dd2def3a55fd0bf481a35a0765eeacfa20b6f/480w.png"
thumbnail_dimensions = [480, 480]
channel = "nightly"
-->


Display air traffic data kindly provided by [INVOLI](https://www.involi.com).

<picture>
  <img src="https://static.rerun.io/air_traffic_data/4a68b46a404c4f9e3c082f57a8a8ed4bf5b9b236/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/air_traffic_data/4a68b46a404c4f9e3c082f57a8a8ed4bf5b9b236/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/air_traffic_data/4a68b46a404c4f9e3c082f57a8a8ed4bf5b9b236/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/air_traffic_data/4a68b46a404c4f9e3c082f57a8a8ed4bf5b9b236/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/air_traffic_data/4a68b46a404c4f9e3c082f57a8a8ed4bf5b9b236/1200w.png">
</picture>

This example demonstrates multiple aspects of the Rerun viewer:

- Use of the [map view](https://rerun.io/docs/reference/types/views/map_view).
- Use of [pyproj](https://pyproj4.github.io/pyproj/stable/) to transform geospatial data from one coordinate system to another.
- Use [GeoPandas](https://geopandas.org/en/stable/) to load geospatial data into a 3D view.
- Use [Polars]https://pola.rs) to batch data to be sent via [`rr.send_columns()`](https://rerun.io/docs/howto/logging/send-columns) (use `--batch`).


## Run the code

To run this example, make sure you have Python version at least 3.9, the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/air_traffic_data
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m air_traffic_data
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m air_traffic_data --help
```
