---
title: Migration to 0.9
order: 10
---

## Overview

Rerun-0.9 introduces a new set of object-oriented logging APIs built on top of an updated, more concrete,
[data model](../concepts/entity-component.md).

Rather than using different functions to log different kinds of data, all data logging now goes through a singular `log`
function. The easiest way to use the `log` function is with the Rerun-provided "Archetypes."

Archetypes are a newly introduced concept in the data model to go alongside "Components" and "DataTypes." Archetypes
represent common objects that are natively understood by the viewer, e.g. `Image` or `Points3D`. Every legacy logging
API has been replaced by one (or more) new Archetypes. You can find all of the available archetypes in the
[Archetype Overview](data_types/archetypes.md). All Archetypes are part of the top-level `rerun` namespace.

In practice, the changes are mostly demonstrated in the following example:

code-example: log_line

Note that for any Archetype that supports batching the objects are now plural. For example, points are now logged
with the `Points3D` archetype. Even if you are logging a single point, under the hood it is always implemented as a
batch of size 1.

For more information on the relationship between Archetypes, Components, and DataTypes, please see our guide to the [Rerun Data Model](../concepts/entity-component.md).

## Migrating Python Code

...
