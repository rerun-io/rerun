<!--[metadata]
title = "PX4 ulog"
tags = ["2D", "3D", "map", "drone"]
description = "Load and visualize a PX4 ulog drone log file"
-->


<!--
TODO:
thumbnail = "https://static.rerun.io/air_traffic_data/348dd2def3a55fd0bf481a35a0765eeacfa20b6f/480w.png"
thumbnail_dimensions = [480, 480]
#channel = "nightly"
-->

TODO: complete thse

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
