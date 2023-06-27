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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/b1ccd4e1fd2f1fba53ee81de89b02186dd8f76ad_arrow3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ef88127921ab6019465c866dfa562036d6d0325c_arrow3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/1489a02534f9b0afedb6abb6ee222932cfdb2be5_arrow3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/23342496350bd104617afdb4782cd95db21cab06_arrow3d_simple_1200w.png">
  <img src="https://static.rerun.io/c8a8b1cbca40acdf02fb5bf264658ad66e07ca40_arrow3d_simple_full.png" alt="">
</picture>
