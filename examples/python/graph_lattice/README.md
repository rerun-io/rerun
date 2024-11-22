<!--[metadata]
title = "Graph Lattice"
tags = ["Graph", "Layout"]
thumbnail = "https://static.rerun.io/graph_lattice/f53a939567970272cf7c740f1efe5c72f20de7ab/480w.png"
thumbnail_dimensions = [480, 359]
channel = "main"
-->

This example shows different attributes that you can associate with nodes in a graph.
Since no explicit positions are passed for the nodes, Rerun will layout the graph automatically.

<picture>
  <img src="https://static.rerun.io/graph_lattice/35d4018767317e58f63e89c41dee1b71d7573900/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/graph_lattice/35d4018767317e58f63e89c41dee1b71d7573900/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/graph_lattice/35d4018767317e58f63e89c41dee1b71d7573900/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/graph_lattice/35d4018767317e58f63e89c41dee1b71d7573900/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/graph_lattice/35d4018767317e58f63e89c41dee1b71d7573900/1200w.png">
</picture>

## Used Rerun types
[`GraphNodes`](https://www.rerun.io/docs/reference/types/archetypes/graph_nodes),
[`GraphEdges`](https://www.rerun.io/docs/reference/types/archetypes/graph_edges)

## Run the code

```bash
pip install -e examples/python/graph_lattice
python -m graph_lattice
```
