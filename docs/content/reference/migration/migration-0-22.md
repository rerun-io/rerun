---
title: Migrating from 0.20 to 0.21
order: 989
---

### Previously deprecated `DisconnectedSpace` archetype/component got now removed.

The deprecated `DisconnectedSpace` archetype and `DisconnectedSpace` component have been removed.
To achieve the same effect, you can log any of the following "invalid" transforms:
* zeroed 3x3 matrix
* zero scale
* zeroed quaternion
* zero axis on axis-angle rotation

Previously, the `DisconnectedSpace` archetype played a double role by governing view spawn heuristics & being used as a transform placeholder.
This led to a lot of complexity and often broke or caused confusion (see https://github.com/rerun-io/rerun/issues/6817, https://github.com/rerun-io/rerun/issues/4465, https://github.com/rerun-io/rerun/issues/4221).
By now, explicit blueprints offer a better way to express which views should be spawned and what content they should query.
(you can learn more about blueprints [here](https://rerun.io/docs/getting-started/configure-the-viewer/through-code-tutorial)).
