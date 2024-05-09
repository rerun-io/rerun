<!--[metadata]
title = "Stock chart (time range)"
tags = ["Time series", "Blueprint"]
thumbnail = "https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/480w.png"
thumbnail_dimensions = [480, 270]
-->

This example fetches the last 5 days of stock data for a few different stocks.
We show how Rerun blueprints can then be used to present many different views of the same data.

This is an alternative version of the blueprint_stocks example that uses time ranges to create the daily
time series for each stock. This allows the underlying data to be stored on a single entity rather than
splitting it across multiple entities for each day.

<picture>
  <img src="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_stocks/8bfe6f16963acdceb2debb9de9a206dc2eb9b280/1200w.png">
</picture>


```bash
pip install -e examples/python/blueprint_stocks_timerange
```

```bash
python -m blueprint_stocks_timerange
```

The different blueprints can be explored using the `--blueprint` flag. For example:

```
python -m blueprint_stocks_timerange --blueprint=compare
```

Available choices are:

-   `one`: Uses a filter and time range to show only a single chart.
-   `compare`: Compares two stocks on several different days.
-   `grid`: Shows all the charts in a grid layout.
