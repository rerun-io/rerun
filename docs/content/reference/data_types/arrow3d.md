---
title: Arrow3D
order: 1
---
`Arrow3D` represents a singular arrow in three-dimensional space. The `arrow3d` component is defined by an origin, and a
vector relative to that origin. The arrow tip will be drawn at the end of the vector, pointing away from the origin.
Additionally, arrows can be drawn with color, radius, and labels. The radius controls the thickness of the arrow.

Notes:
* In the python APIs `radius` is currently derived from `width_scale`
* [Arrow APIs do not currently support batching](https://github.com/rerun-io/rerun/issues/2466)

## Components and APIs
Primary component: `arrow3d`

Secondary components: `colorrgba`, `radius`, `label`

Python APIs: [log_arrow](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_arrow)

Rust API: [Arrow3D](https://docs.rs/rerun/latest/rerun/components/struct.Arrow3D.html)

## Simple Example

code-example: arrow3d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/3f8a63c38f2e3b5dc0389a87a7760fb5931af06c_arrow3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/09d2af7cb9e274c120025851baee99cd72f01831_arrow3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/5e7e3b1f9117bdc546ed516ed82b4579a13b9c8a_arrow3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/8524cc65f1d84a26b2808400aa1d64224b1fd4da_arrow3d_simple_1200w.png">
  <img style="width: 75%" src="https://static.rerun.io/1505438a73ca779a6e2c3c3b3b41f67ce62c724b_arrow3d_simple_full.png" alt="">
</picture>
