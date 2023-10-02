---
title: "DisconnectedSpace"
---

Specifies that the entity path at which this is logged is disconnected from its parent.

This is useful for specifying that a subgraph is independent of the rest of the scene.

If a transform or pinhole is logged on the same path, this archetype's components
will be ignored.

## Components

**Required**: [`DisconnectedSpace`](../components/disconnected_space.md)

## Example

### disconnected_space

code-example: disconnected_space

