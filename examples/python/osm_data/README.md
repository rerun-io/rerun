<!--[metadata]
title = "OSM data"
tags = ["Map", "Blueprint"]
thumbnail_dimensions = [480, 480]
thumbnail = "https://static.rerun.io/osm_data/0be94071469c49f98326d85456ed2a3af8d1733a/480w.png"
channel = "release"
-->


Download [`OpenStreetMap`](https://www.openstreetmap.org) data via the [Overpass](https://overpass-api.de) API and [query language](https://wiki.openstreetmap.org/wiki/Overpass_API/Overpass_QL),
and display it on a [map view](https://www.rerun.io/docs/reference/types/view/map_view?speculative-link).

<picture>
  <img src="https://static.rerun.io/osm-data/926e89e0587b0d66a1cd620b3f5b77ac79eca272/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/osm-data/926e89e0587b0d66a1cd620b3f5b77ac79eca272/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/osm-data/926e89e0587b0d66a1cd620b3f5b77ac79eca272/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/osm-data/926e89e0587b0d66a1cd620b3f5b77ac79eca272/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/osm-data/926e89e0587b0d66a1cd620b3f5b77ac79eca272/1200w.png">
</picture>

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
pip install -e examples/python/osm_data
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m osm_data # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m osm_data --help
```
