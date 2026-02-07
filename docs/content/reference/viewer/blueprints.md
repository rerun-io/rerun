---
title: Blueprint Panel
order: 1
---

The Blueprint Panel shows the hierarchical structure of your current blueprint and provides controls for modifying the Viewer layout.

For a complete understanding of blueprints, see [Blueprints](../../concepts/visualization/blueprints.md). For hands-on tutorials on configuring the Viewer, see [Configure the Viewer](../../getting-started/configure-the-viewer.md).

<picture>
  <img src="https://static.rerun.io/blueprint-example/24fe3f15c15dc8c74e1feec879cab624a34136e6/full.png" alt="Blueprint panel showing view hierarchy">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint-example/24fe3f15c15dc8c74e1feec879cab624a34136e6/480w.png">
</picture>

## Panel header controls

### Reset button
The reset button in the blueprint panel header provides two reset options:

- **Reset to Default Blueprint**: Returns to your programmatically specified blueprint (sent from code via `rr.send_blueprint()`) or a loaded blueprint file (`.rbl`). This becomes the "default" whenever you send or load a blueprint.

- **Reset to Heuristic Blueprint**: Generates a new blueprint automatically based on your current data. The Viewer analyzes your logged data and creates an appropriate layout using built-in heuristics.

If no default blueprint has been set, the reset button uses the heuristic blueprint. See [Reset Behavior](../../concepts/visualization/blueprints.md#reset-behavior-heuristic-vs-default) for more details.

### Add view
The "+" button allows you to add new views or containers.

## Blueprint tree

The blueprint panel displays a tree view showing:
- The viewport (root container)
- Nested containers (Horizontal, Vertical, Grid, Tabs)
- Views within containers
- Entities displayed in each view

### Interaction

Hovering over any item reveals controls for:
- **Eye icon**: Show or hide the item
- **"-" button**: Remove the item from the blueprint

Right-click any item for a context menu with additional operations. See [Configure the Viewer](../../getting-started/configure-the-viewer.md#interactive-configuration) for details on all interactive operations.

### Data blueprints

Entities shown in the blueprint panel refer to their *data blueprints*—the entity plus its associated blueprint settings. Changes made here apply only to the specific view where the entity appears.

### Groups

When entities are added to a view (manually or automatically), hierarchical groupings are created based on [Entity Paths](../../concepts/logging-and-ingestion/entity-path.md). These groups help organize large views and enable hierarchical manipulation of blueprints. Groups are independent of logged data and exist purely for blueprint organization.

## Adding entities to views

To add or re-add an entity to a view:
1. Select the target view in the blueprint panel
2. Click the button in the [Selection Panel](selection.md) to open the entity picker
3. Select entities to add (only compatible entities for that view type are shown)

See [Entity Queries](../../concepts/visualization/entity-queries.md) for information on how view content is determined.

For more information about configuring the viewer, see [Configure the Viewer](../../getting-started/configure-the-viewer.md).
