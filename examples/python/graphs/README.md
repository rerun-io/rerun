<!--[metadata]
title = "Graphs"
tags = ["Graph", "Layout", "Node-link diagrams", "Bubble charts"]
thumbnail = "https://static.rerun.io/graphs/1b93c00867821ebf3286653f43a9e5eb993f59ff/480w.png"
thumbnail_dimensions = [480, 399]
channel = "main"
-->

This example shows different types of graphs (and layouts) that you can visualize using Rerun.

<picture>
  <img src="https://static.rerun.io/graphs/1b93c00867821ebf3286653f43a9e5eb993f59ff/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/graphs/1b93c00867821ebf3286653f43a9e5eb993f59ff/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/graphs/1b93c00867821ebf3286653f43a9e5eb993f59ff/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/graphs/1b93c00867821ebf3286653f43a9e5eb993f59ff/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/graphs/1b93c00867821ebf3286653f43a9e5eb993f59ff/1200w.png">
</picture>

Rerun ships with an integrated engine to produce [force-based layouts](https://en.wikipedia.org/wiki/Force-directed_graph_drawing) to visualize graphs.
Force-directed layout approaches have to advantage that they are flexible and can therefore be used to create different kinds of visualizations.
This example shows different types of layouts:

* Regular force-directed layouts of node-link diagrams
* Bubble charts, which are based on packing circles

## Used Rerun types

[`GraphNodes`](https://www.rerun.io/docs/reference/types/archetypes/graph_nodes?speculative-link),
[`GraphEdges`](https://www.rerun.io/docs/reference/types/archetypes/graph_edges?speculative-link)

## Force-based layouts

To compute the graph layouts, Rerun implements a physics-simulation that is very similar to [`d3-force`](https://d3js.org/d3-force). In particular, we implement the following forces:

* Centering force, which shifts the center of mass of the entire graph.
* Collision radius force, which resolves collisions between nodes in the graph, taking their radius into account.
* Many-Body force, which can be used to create attraction or repulsion between nodes.
* Link force, which acts like a spring between two connected nodes.
* Position force, which pull all nodes towards a given position, similar to gravity.

If you want to learn more about these forces, we recommend looking at the [D3 documentation](https://d3js.org/d3-force) as well.

The physics simulation that we implemented called _Fjädra_ and can be found on [GitHub](https://github.com/grtlr/fjadra) and on [`crates.io`](https://crates.io/crates/fjadra).

## Run the code

```bash
pip install -e examples/python/graphs
python -m graphs
```

