<!--[metadata]
title = "RRT*"
tags = ["2D"]
thumbnail= "https://static.rerun.io/rrt-star/fbbda33bdbbfa469ec95c905178ac3653920473a/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
-->

This example visualizes the path finding algorithm RRT\* in a simple environment.

<picture>
  <img src="https://static.rerun.io/rrt-star/4d4684a24eab7d5def5768b7c1685d8b1cb2c010/full.png" alt="RRT* example screenshot">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rrt-star/4d4684a24eab7d5def5768b7c1685d8b1cb2c010/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rrt-star/4d4684a24eab7d5def5768b7c1685d8b1cb2c010/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rrt-star/4d4684a24eab7d5def5768b7c1685d8b1cb2c010/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rrt-star/4d4684a24eab7d5def5768b7c1685d8b1cb2c010/1200w.png">
</picture>

A detailed explanation can be found in the original paper
Karaman, S. Frazzoli, S. 2011. "Sampling-based algorithms for optimal motion planning".
or in [this medium article](https://theclassytim.medium.com/robotic-path-planning-rrt-and-rrt-212319121378)

```bash
pip install -r examples/python/rrt-star/requirements.txt
python examples/python/rrt-star/main.py
```
