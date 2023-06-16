---
title: Image
order: 20
---

## Components and APIs
## Components and APIs
Primary component: `tensor`

Secondary components: `colorrgba`, `draw_order`

Python APIs: [log_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image**), [log_image_file](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image_file**),

Rust API: [Tensor](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html)

`colorrgba` is currently only supported for images,
i.e. tensors with 2 dimensions and an optional 3rd that can be interpreted as color channels.
Furthermore, only the spatial Space View is able to use the color component.
