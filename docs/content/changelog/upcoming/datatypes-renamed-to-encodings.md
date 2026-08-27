---
title: "`datatypes` renamed to `encodings`"
hidden: true
type: breaking
---

### `datatypes` renamed to `encodings`

`datatype` meant two different things in Rerun: the low-level types that components are built from, and the Arrow `DataType` those are stored as.
The first of the two is now called an **encoding**, so `rerun.datatypes` is `rerun.encodings`.

The old spelling still works, deprecated, and will be removed in a future release.

Python:

```py
rr.datatypes.Vec3D([1, 2, 3])  # before
rr.encodings.Vec3D([1, 2, 3])  # after
```

Rust:

```rust
use re_types::datatypes::Vec3D;  // before
use re_types::encodings::Vec3D;  // after
```

C++ — the namespace alias keeps working, but the per-type include paths moved:

```cpp
#include <rerun/datatypes/vec3d.hpp>  // before
#include <rerun/encodings/vec3d.hpp>  // after
```

Reference pages moved from `reference/types/datatypes/…` to `reference/types/encodings/…`; the old URLs redirect.

Docs: ../reference/types/encodings.md
