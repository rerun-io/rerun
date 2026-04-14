---
title: Migrating from 0.31 to 0.32
order: 978
---

## Lenses API (Rust)

### `output_columns_at` / `output_scatter_columns_at` replaced by `OutputBuilder::at_entity`

The `output_columns_at` and `output_scatter_columns_at` methods on `LensBuilder` have been removed.
Use `at_entity` on the `OutputBuilder` inside the closure instead:

```rust
// Before
Lens::for_input_column(filter, "component")
    .output_columns_at("target/entity", |out| {
        out.component(descriptor, selector)
    })?

// After
Lens::for_input_column(filter, "component")
    .output_columns(|out| {
        out.at_entity("target/entity")
            .component(descriptor, selector)
    })?
```

The same applies to scatter columns:

```rust
// Before
.output_scatter_columns_at("target/entity", |out| { … })?

// After
.output_scatter_columns(|out| {
    out.at_entity("target/entity") // …
})?
```

Additionally, `ColumnsBuilder` and `ScatterColumnsBuilder` have been unified into a single `OutputBuilder` type.
