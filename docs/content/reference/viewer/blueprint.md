---
title: Blueprint
order: 1
---

The blueprint is how you configure what is displayed in the Rerun viewer.
It is saved between sessions and is unique to a given [application id](../../concepts/apps-and-recordings.md).
The Blueprint includes all Space View configurations, Entity groupings and Entity settings, also known as Data Blueprints.

This view shows the Blueprint for the active recording.
Everything visible in the [Viewport](viewport.md) has a representation here,
making it an easy way to select a Space View and the [Entities](../../concepts/entity-component.md) it shows.

<picture>
  <img src="https://static.rerun.io/blueprint-view/d01d0f5baf46d56f32925f9b10d793d1495a3a39/full.png" alt="screenshot of the blueprint view">
</picture>


Controls
--------
### Reset
The reset button resets the entire Blueprint back to its heuristic-chosen default.
This includes all settings for Entities, Groups and Space Views.

### Add Space View
With this control you can add new Space Views for arbitrary [Spaces](../../concepts/spaces-and-transforms.md).

Contents
--------
Upon hovering any line in the Blueprint panel, you'll find shorthands for removing and hide/show.

### Data Blueprints
All Entities shown in the blueprint panel refer in fact to their Data Blueprints.
I.e. the entity plus the associated blueprint settings.
As such, all changes made here are only relevant for the Space View in which they reside.

### Groups
Whenever Entities are added to a Space View (either manually or automatically), groupings
are automatically created.
Groups, despite being derived from the [Entity Path](../../concepts/entity-path.md) are independent of logged data.
They are meant to improve the handling of large Space Views and allow for hierarchical manipulation
of blueprints.

Adding Entities
-----------------------------
To (re-)add an Entity to a Space View, you need first need to select the respective Space View.
You then can open a dedicated menu through a button in the [Selection view](selection.md).

This allows you to add any Entity with a matching [category](viewport.md#Categories-of-Space-Views) and a valid [transform](../../concepts/spaces-and-transforms.md) to your
Space View's path.
