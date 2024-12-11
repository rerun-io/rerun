---
title: Blueprint
order: 1
---

The blueprint is how you configure what is displayed in the Rerun viewer.
It is saved between sessions and is unique to a given [application id](../../concepts/apps-and-recordings.md).
The blueprint includes all view configurations, entity groupings and entity settings, also known as _data blueprints_.

This view shows the blueprint for the active recording.
Everything visible in the [Viewport](viewport.md) has a representation here,
making it an easy way to select a View and the [Entities](../../concepts/entity-component.md) it shows.

<picture>
  <img src="https://static.rerun.io/blueprint-example/24fe3f15c15dc8c74e1feec879cab624a34136e6/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint-example/24fe3f15c15dc8c74e1feec879cab624a34136e6/480w.png">
</picture>


Controls
--------
### Reset
The reset button resets the entire blueprint back to its heuristic-chosen default.
This includes all settings for entities, Groups and Views.

### Add view
With this control you can add new Views for arbitrary [Spaces](../../concepts/spaces-and-transforms.md).

Contents
--------
Upon hovering any line in the Blueprint panel, you'll find shorthands for removing and hide/show.

### Data blueprints
All entities shown in the blueprint panel refer in fact to their Data Blueprints.
I.e. the entity plus the associated blueprint settings.
As such, all changes made here are only relevant for the View in which they reside.

### Groups
Whenever entities are added to a view (either manually or automatically), groupings
are automatically created.
Groups, despite being derived from the [Entity Path](../../concepts/entity-path.md) are independent of logged data.
They are meant to improve the handling of large views and allow for hierarchical manipulation
of blueprints.

Adding Entities
-----------------------------
To (re-)add an entity to a view, you need first need to select the respective view.
You then can open a dedicated menu through a button in the [Selection view](selection.md).

This allows you to add any entity with a matching [category](viewport.md#view-classes) and a valid [transform](../../concepts/spaces-and-transforms.md) to your
view's path.
