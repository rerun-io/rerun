---
title: Configuring the Viewer
order: 10
---

Although the Rerun Viewer tries to do a reasonable job of using heuristics to automatically determine
an appropriate layout given the data that you provide, there will always be situations where the heuristic
results don't match the needs of a particular use-case.

Fortunately, almost all aspects of the viewer can be configured via the [Blueprint](../concepts/blueprint.md).

The viewer Blueprint completely determines:

-   What contents are included in each view
-   The type of view used to visualize the data
-   The organizational layout and names of the different view panels and containers
-   Configuration and styling properties of the individual data visualizers

There are a few different ways to work with Blueprints:

-   [Blueprints can be modified directly through the UI](./configuring-the-viewer/blueprint-ui.md)
-   [Blueprint files (.rbl) can be saved and loaded from disk](./configuring-the-viewer/rbl-files.md)
-   [Blueprints can be generated direct from code](./configuring-the-viewer/blueprint-apis.md)

For a hands on example, you can also follow the [Blueprint API Tutorial](./configuring-the-viewer/blueprint-api-tutorial.md).
