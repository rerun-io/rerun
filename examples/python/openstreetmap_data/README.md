<!--[metadata]
title = "OpenStreetMap data"
tags = ["Map", "Blueprint"]
thumbnail_dimensions = [480, 480]
thumbnail = "https://static.rerun.io/osm_data/0be94071469c49f98326d85456ed2a3af8d1733a/480w.png"
# channel = "release" # disabled since the openstreetmap API is flaky
# include_in_manifest = true
-->


Download [`OpenStreetMap`](https://www.openstreetmap.org) data via the [Overpass](https://overpass-api.de) API and [query language](https://wiki.openstreetmap.org/wiki/Overpass_API/Overpass_QL),
and display it on a [map view](https://www.rerun.io/docs/reference/types/views/map_view).

<picture>
  <img src="https://static.rerun.io/openstreetmap_data/5da23e9244d5cfead76ad484d09ba70cf62c4e57/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/openstreetmap_data/5da23e9244d5cfead76ad484d09ba70cf62c4e57/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/openstreetmap_data/5da23e9244d5cfead76ad484d09ba70cf62c4e57/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/openstreetmap_data/5da23e9244d5cfead76ad484d09ba70cf62c4e57/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/openstreetmap_data/5da23e9244d5cfead76ad484d09ba70cf62c4e57/1200w.png">
</picture>

## Run the code

To run this example, make sure you have the [required Python version](https://ref.rerun.io/docs/python/main/common#supported-python-versions), the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/openstreetmap_data
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m openstreetmap_data # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m openstreetmap_data --help
```
