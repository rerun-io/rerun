<!--[metadata]
title = "Air Traffic Data"
tags = ["2d", "3d", "map", "crs"]
description = "Display aircraft flight trajectories"
build_args = ["--jpeg-quality=50"]
-->


Display air traffic data kindly provided by [INVOLI](https://www.involi.com).


```bash
# install dependencies
pip install -r examples/python/air_traffic_data/requirements.txt

# run with demo dataset
python examples/python/air_traffic_data/main.py --dataset 10min

# run with custom dataset
python examples/python/air_traffic_data/main.py --dir path/to/my/dataset/directory

# more options
python examples/python/air_traffic_data/main.py --help
```
