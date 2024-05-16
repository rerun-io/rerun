<!--[metadata]
title = "Stock charts"
tags = ["Time series", "Blueprint"]
thumbnail = "https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/480w.png"
thumbnail_dimensions = [480, 270]
-->

This example fetches the last 5 days of stock data for a few different stocks.
We show how Rerun blueprints can then be used to present many different views of the same data.

<picture>
  <img src="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/1200w.png">
</picture>


```bash
pip install -e examples/python/blueprint_stocks
python -m blueprint_stocks
```

The different blueprints can be explored using the `--blueprint` flag. For example:

```
python -m blueprint_stocks --blueprint=one-stock
```

Available choices are:

-   `auto`: Reset the blueprint to the auto layout used by the viewer.
-   `one-stock`: Uses a filter to show only a single chart.
-   `one-stock-with-info`: Uses a container to layout a chart and its info document
-   `one-stock-no-peaks`: Uses a filter to additionally remove some of the data from the chart.
-   `compare-two`: Adds data from multiple sources to a single chart.
-   `grid`: Shows all the charts in a grid layout.
