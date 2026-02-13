<!--[metadata]
title = "Minimal example"
thumbnail = "https://static.rerun.io/minimal-example/9e694c0689f20323ed0053506a7a099f7391afca/480w.png"
thumbnail_dimensions = [480, 480]
tags = ["3D", "API example"]
-->

Generates a 3D colored cube and demonstrates how to log a point cloud.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/1200w.png">
  <img src="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/full.png" alt="Minimal example screenshot">
</picture>

Straightforward example from the [Quick Start guide](https://www.rerun.io/docs/getting-started/data-in/python) to generate a 3D colored cube and demonstrate how to log a point cloud.

## Used Rerun types

[`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d)

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

It logs 3D points, each associated with a specific color, forming a grid pattern using [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype.
```python
import rerun as rr
import numpy as np

rr.init("rerun_example_my_data", spawn=True)

SIZE = 10

pos_grid = np.meshgrid(*[np.linspace(-10, 10, SIZE)]*3)
positions = np.vstack([d.reshape(-1) for d in pos_grid]).T

col_grid = np.meshgrid(*[np.linspace(0, 255, SIZE)]*3)
colors = np.vstack([c.reshape(-1) for c in col_grid]).astype(np.uint8).T

rr.log(
    "my_points",
    rr.Points3D(positions, colors=colors, radii=0.5)
)
 ```

## Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/minimal
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m minimal # run the example
```
