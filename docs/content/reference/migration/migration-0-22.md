---
title: Migrating from 0.21 to 0.22
order: 988
---

### Previously deprecated `DisconnectedSpace` archetype/component got now removed

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


### Removed `num_instances` keyword argument to `rr.log_components()`

For historical reasons, the `rr.log_components()` function of the Python SDK accepts an optional, keyword-only argument `num_instances`.
It was no longer used for several releases, so we removed it.

**Note**: although `rr.log_components()` is technically a public API, it is undocumented, and we discourage using it.
For logging custom components, use [`rr.AnyValue`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyValues) and [`rr.AnyBatchValue`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyBatchValue).


### Rust's `ViewCoordinates` archetype now has static methods instead of constants

As part of the switch to "eager archetype serialization" (serialization of archetype components now occurs at time if archetype instantiation rather than logging), we can no longer offer constants
for the `ViewCoordinates` archetype like `ViewCoordinates::RUB`.
Instead, there's now methods with the same name, i.e. `ViewCoordinates::RUB()`.
