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

Note that for any Archetype that supports batching the object names are now plural. For example, points are now logged
with the `Points3D` archetype. Even if you are logging a single point, under the hood it is always implemented as a
batch of size 1.

For more information on the relationship between Archetypes, Components, and DataTypes, please see our guide to the [Rerun Data Model](../concepts/entity-component.md).

## Migrating Python Code

All of the previous `log_*` functions have been marked as deprecated and will be removed in `0.10`. We have done our
best to keep these functions working as thin wrappers on top of the new logging APIs, though there may be subtle
behavioral differences.

### The log module has become the log function
This is one area where we were forced to make breaking changes.  Rerun previously had an internal `log` module where the
assorted log-functions and helper classes were implemented. In general, these symbols were all re-exported to the
top-level `rerun` namespace.  However, in some cases these fully-qualified paths were used for imports. Because
`rerun.log` is now a function rather than a module, any such imports will result in an import error. Look for the
corresponding symbol in the top-level `rerun` namespace instead.

### Updating to the log APIs

In most cases migrating your code to the new APIs should be straightforward. The legacy functions have been marked as
deprecated and the deprecation warning should point you to the correct Archetype to use instead.  Additionally, in most
cases, the old parameter names match the parameters taken by the new Archetype constructors, though exceptions are noted below.

#### `log_point`, `log_points`
Can be replaced with [Points2D](data_types/archetypes/points2d.md) or [Points3D](data_types/archetypes/points3d.md).

Relevant Python docs:
 - [Points2D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Points2D.__init__)
 - [Points3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Points3D.__init__)

Notes:
 - `stroke_width` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`

#### `log_rect`, `log_rects`
Can be replaced with [Boxes2D](data_types/archetypes/boxes2d.md)

Relevant Python docs:
 - [Boxes2D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Boxes2D.__init__)

Notes:
 - Can now be constructed with 2 arrays: one of `sizes`, and the other of either `half_sizes` o `sizes`
 - The legacy behavior can be matched by instead using the params `array` and `array_format`. `array_format` takes an
   `rr.Box2DFormat`.
 - `identifiers` has become `instance_keys`

