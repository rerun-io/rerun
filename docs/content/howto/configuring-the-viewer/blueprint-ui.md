---
title: Blueprint UI
order: 1
---

The Rerun Viewer is configurable directly through the UI itself.

## Viewer overview

TODO(#5636): Screenshot of the viewer, with the blueprint and selection panels highlighted.

The left panel of the viewer is the "Blueprint Panel" this shows a visual tree view representing
the contents of the current blueprint.

The right panel of the viewer is the "Selection Panel" this panels allows you to configure
specific blueprint properties of the currently selected element.

After editing the viewer you may want to [save or share the blueprint](./rbl-files.md).

## Configuring Layout and Contents

### Show or hide parts of the blueprint

Click the "eye" icon next to the container, view, or entity in the blueprint panel.
TODO(#5636): show_hide

### Add new Containers or Views

Clicking the "+" at the top of the blueprint panel.
TODO(#5636): add_1

Selecting a Container and clicking the "+" in the selection panel.
TODO(#5636): add_2

### Remove a View or Container

Click the "-" button next to the container or view in the blueprint panel.
TODO(#5636): remove

### Re-arrange existing Containers or Views

Drag and drop the container or view in the blueprint panel.
TODO(#5636): drag_1

Drag and drop containers or views directly in the viewport
TODO(#5636): drag_2

### Change the size of Contaainers or Views

Click and drag the edge of a view or container to resize it
TODO(#5636): resize

### Rename a View or Container

Select the Container or View and enter a name at the top of the selection panel
TODO(#5636): rename

### Change the type of a Container

Select the Container and choose a new type from the dropdown in the selection panel
TODO(#5636): change_type

### Add a new Data to a View

Select the view and click "edit" to bring up the entity editor
TODO(#5636): add_data_1

Select the view and directly edit the entity query
See [Entity Queries](../../concepts/entity-queries.md) for more information on how to write queries.

TODO(#5636): add_data_2

### Remove Data from a View

Click the "-" next to the entity in the blueprint panel
TODO(#5636): remove_data_1

### Change the origin of a View

Select the view, then click the "Space Origin" field and type or select a new origin
TODO(#5636): change_origin

## Overriding Properties

TODO(jleibs): do we include this now or wait for generalized component overrides?
