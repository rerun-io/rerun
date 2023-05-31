---
title: Selection
order: 2
---

Making selections is one of the primary ways of exploring data in Rerun.
The current selection can be changed with a mouse click on most elements of the - 
including the [Blueprint](blueprint.md), [Viewport](viewport.md),
[Timeline](timeline.md), [Event Log](overview.md#event-log)
and even the Selection view itself.


Parts of the Selection view
---------------------------

![selection overview](/docs-media/selection-overview.png)

### Selection history
Rerun keeps a log of all your selections, allowing you to undo/redo previous selections
with the ←/→ buttons at the top of the view or `ctrl + shift + left/right`.

Right clicking on the buttons expands the full history

### What is selected
Here you find what is selected and for some objects in which context.
This context not only gives you a convenient way to jump to related objects,
but is also important for what the following sections are showing.

### Data & Blueprint sections
The data section always shows static, raw user logged data for the currently selected time.
Some objects, e.g. Space Views, may not have a data section and expose only Blueprint options.

In contrast, the Blueprint section is timeline independent and exposes the
[Blueprint settings](blueprint.md) of an Entity in the context of a given Space View.
To learn more about the various settings check the on-hover tooltips.

Click-through selections
------------------------
Making selections can be context sensitive to the current selection.
The most common case for this is selecting instances of an entity (see also [Batch Data](../../concepts/batches.md)):
E.g. in order to select a point of a point cloud in a Space View,
first select the entire entity (the cloud) by clicking on one of the points.
Once the cloud is selected, you can further refine that selection by clicking on an individual point.

Multi Selection
---------------
By holding `cmd/ctrl` upon click, you can add or remove selections from the set of currently selected objects.
The selection view shows all selected objects in the order they were added.
