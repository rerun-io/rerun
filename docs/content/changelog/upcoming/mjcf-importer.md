---
title: Import MuJoCo (MJCF) models
hidden: true
type: feature
---

### Import MuJoCo (MJCF) models

Rerun can now open [MuJoCo](https://mujoco.org/) models from `.mjcf` files and from `.xml` files with a `<mujoco>` root.
Heightfields, SDF geoms, textures, and the `<replicate>`, `<composite>` and `<flexcomp>` elements are not supported yet.

Like with URDF, every body becomes a named frame, and the new `rerun.mjcf.MjcfTree` (`re_importer::MjcfTree` in Rust) computes the transforms that animate them from joint values, using MuJoCo's `qpos` convention.
Hinge and slide joints are supported.
A body with several joints gets an intermediate frame after each joint but its last, named `{body}::{joint}`, so that every joint can be animated on its own.

See [Loading MJCF models](../logging-and-ingestion/import/mjcf.md).
