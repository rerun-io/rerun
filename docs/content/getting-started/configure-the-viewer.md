---
title: Configure the viewer
order: 600
---

By default, the Rerun Viewer uses heuristics to automatically determine an appropriate
layout given the data that you provide. However, there will always be situations
where the heuristic results don't match the needs of a particular use-case.

Fortunately, almost all aspects of the Viewer can be configured via the [Blueprint](../reference/viewer/blueprint.md).

The Viewer Blueprint completely determines:

-   What contents are included in each view
-   The type of view used to visualize the data
-   The organizational layout and names of the different view panels and containers
-   Configuration and styling properties of the individual data visualizers

There are a few different ways to work with Blueprints:

-   [Blueprints can be modified interactively](./configure-the-viewer/interactively.md)
-   [Blueprint files (.rbl) can be saved and loaded from disk](./configure-the-viewer/save-and-load.md)
-   [Blueprints can be controlled directly from code](./configure-the-viewer/through-code-tutorial.md)
