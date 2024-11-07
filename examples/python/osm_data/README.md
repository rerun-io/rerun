<!--[metadata]
title = "OSM Data"
tags = ["Map", "Blueprint"]
thumbnail_dimensions = [480, 480]
-->

<!-- TODO(ab): add this to frontmatter

thumbnail = "https://static.rerun.io/nuscenes/9c50bf5cadb879ef818ac3d35fe75696a9586cb4/480w.png"
channel = "release"
-->

Download [`OpenStreetMap`](https://www.openstreetmap.org) data via the [Overpass](https://overpass-api.de) API and display it on a [map view](https://www.rerun.io/docs/reference/types/view/map_view).

<!-- TODO(ab): screenshot -->


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
