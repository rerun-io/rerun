<!--[metadata]
title = "Plots"
tags = ["2D", "plots", "api-example"]
description = "Demonstration of various plots and charts supported by Rerun."
thumbnail = "https://static.rerun.io/plots/e8e51071f6409f61dc04a655d6b9e1caf8179226/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
-->


<picture data-inline-viewer="examples/plots">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/1200w.png">
  <img src="https://static.rerun.io/plots/c5b91cf0bf2eaf91c71d6cdcd4fe312d4aeac572/full.png" alt="Plots example screenshot">
</picture>

This example demonstrates how to log simple plots with the Rerun SDK. Charts can be created from 1-dimensional tensors, or from time-varying scalars.

```bash
python examples/python/plots/main.py
```
