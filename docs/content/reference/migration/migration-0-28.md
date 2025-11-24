---
title: Migrating from 0.27 to 0.28
order: 982
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## `Transform3D` no longer supports `axis_length` for visualizing coordinate axes

The `axis_length` parameter/method has been moved from `Transform3D` to a new `TransformAxes3D` archetype, which you can log alongside of `Transform3D`.
This new archetype also works with the `CoordinateFrame` archetype.

Existing `.rrd` recordings will be automatically migrated when opened (the migration converts `Transform3D:axis_length` components to `TransformAxes3D:axis_length`).

## Changes to `Transform3D`/`InstancePose3D` are now treated transactionally by the Viewer

If you previously updated only certain components of `Transform3D`/`InstancePose3D` and relied on previously logged
values remaining present,
you must now re-log those previous values every time you update the `Transform3D`/`InstancePose3D`.

If you always logged the same transform components on every log/send call or used the standard constructor of
`Transform3D`, no changes are required!

snippet: migration/transactional_transforms

### Details & motivation

We changed the way `Transform3D` and `InstancePose3D` are queried under the hood!

Usually, when querying any collection of components with latest-at semantics, we look for the latest update of each
individual component.
This is useful, for example, when you log a mesh and only change its texture over time:
a latest-at query at any point in time gets all the same vertex information, but the texture that is active at any given
point in time may changes.

However, for `Transform3D`, this behavior can be very surprising,
as the typical expectation is that logging a `Transform3D` with only a rotation will not inherit previously logged
translations to the same path.
Previously, to work around this, all SDKs implemented the constructor of `Transform3D` such that it set all components
to empty arrays, thereby clearing everything that was logged before.
This caused significant memory (and networking) bloat, as well as needlessly convoluted displays in the viewer.
With the arrival of explicit ROS-style transform frames, per-component latest-at semantics can cause even more
surprising side effects.

Therefore, we decided to change the semantics of `Transform3D` such that any change to any of its components fully
resets the transform state.

For example, if you change its rotation and scale fields but do not write to translation, we will not look further back
in time to find the previous value of translation.
Instead, we assume that translation is not set at all (i.e., zero), deriving the new overall transform state only from
rotation and scale.
Naturally, if any update to a transform always changes the same components, this does not cause any changes other than
the simplification of not having to clear out all other components that may ever be set, thus reducing memory bloat both
on send and query!
