<!--[metadata]
title = "Graphs"
tags = ["Graph", "Layout"]
thumbnail = "https://static.rerun.io/graphs/19e6ca2e0752cdc7107c10b0cb79a4b7192d9e0b/480w.png"
thumbnail_dimensions = [480, 399]
channel = "main"
-->

This example shows different attributes that you can associate with nodes in a graph.
Since no explicit positions are passed for the nodes, Rerun will layout the graph automatically.

<picture>
  <img src="https://static.rerun.io/graphs/19e6ca2e0752cdc7107c10b0cb79a4b7192d9e0b/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/graphs/19e6ca2e0752cdc7107c10b0cb79a4b7192d9e0b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/graphs/19e6ca2e0752cdc7107c10b0cb79a4b7192d9e0b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/graphs/19e6ca2e0752cdc7107c10b0cb79a4b7192d9e0b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/graphs/19e6ca2e0752cdc7107c10b0cb79a4b7192d9e0b/1200w.png">
</picture>

## Used Rerun types
[`GraphNodes`](https://www.rerun.io/docs/reference/types/archetypes/graph_nodes?speculative-link),
[`GraphEdges`](https://www.rerun.io/docs/reference/types/archetypes/graph_edges?speculative-link)

## Run the code

```bash
pip install -e examples/python/graphs
python -m graphs
```
