---
title: DisconnectedSpace
order: 100
---

Specifies that the entity path at which this is logged is disconnected from its parent.

This is useful for specifying that a subgraph is independent of the rest of the scene.

If a transform or pinhole is logged on the same path, this archetype's components
will be ignored.

## Components and APIs

Required:
* `disconnected_space`

## Examples

### disconnected_space

code-example: disconnected_space

