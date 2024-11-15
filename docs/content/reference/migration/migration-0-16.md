---
title: Migrating from 0.15 to 0.16
order: 994
---


## `timeless` replaced by `static`

The concept of _timeless_ data has been replaced by _static_ data.
Except for the name change, they behave similarly in most use cases.

Static data is data that shows up at all times, on all timelines.

In 0.15, you could log component data to the same entity path using both timeless and temporal data, and the resulting component data would end up being the concatenation of the two.

0.16 introduces static data, which has a far simpler model: if you log static component data to an entity path, it unconditionally overrides any other data (whether static or temporal) for that component.
Once static data has been logged, it can only be overwritten by other static data.

Static data is most often used for `AnnotationContext` and `ViewCoordinates`.


#### C++

```diff
- rec.log_timeless("world", rerun::ViewCoordinates::RIGHT_HAND_Z_UP);
+ rec.log_static("world", rerun::ViewCoordinates::RIGHT_HAND_Z_UP);
```

#### Python

```diff
- rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)
+ rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)
```

#### Rust

```diff
- rec.log_timeless("world", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP)?;
+ rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP)?;
```


## `MeshProperties` replaced by `TriangleIndices`

In PR [#6169](https://github.com/rerun-io/rerun/pull/6169) we replaced `MeshProperties` with `TriangleIndices`. We could do this thanks to simplifications in our data model.

#### C++

```diff
  rerun::Mesh3D(positions)
-     .with_mesh_properties(rerun::components::MeshProperties::from_triangle_indices({{2, 1, 0}}))
+     .with_triangle_indices({{2, 1, 0}})
```

#### Python

```diff
  rr.Mesh3D(
      vertex_positions=…,
-     indices=[2, 1, 0],
+     triangle_indices=[2, 1, 0],
  ),
```

#### Rust

```diff
  rerun::Mesh3D::new(positions)
-     .with_mesh_properties(rerun::MeshProperties::from_triangle_indices([[2, 1, 0]]))
+     .with_triangle_indices([[2, 1, 0]])
```
