<!--[metadata]
title = "Stock Charts"
description = "Uses stock data as an example of how to leverage Rerun blueprints to control the layout and presentation of the viewer."
tags = ["time-series", "blueprint"]
thumbnail = "https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/480w.png"
thumbnail_dimensions = [480, 271]
-->

<picture>
  <img src="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/1200w.png">
</picture>

This example fetches the last 5 days of stock data for a few different stocks.
We show how Rerun blueprints can then be used to present many different views of the same data.

```bash
pip install -r examples/python/blueprint_stocks/requirements.txt
python examples/python/blueprint_stocks/blueprint_main.py
```

The different blueprints can be explored using the `--blueprint` flag. For example:

```
python examples/python/blueprint_stocks/main.py --blueprint=one-stock
```

Available choices are:

-   `auto`: Reset the blueprint to the auto layout used by the viewer.
-   `one-stock`: Uses a filter to show only a single chart.
-   `one-stock-with-info`: Uses a container to layout a chart and its info document.
-   `one-stock-no-peaks`: Uses a filter to additionally remove some of the data from the chart.
-   `compare-two`: Adds data from multiple sources to a single chart.
-   `grid`: Shows all the charts in a grid layout.
