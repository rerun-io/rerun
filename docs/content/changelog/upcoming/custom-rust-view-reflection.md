---
title: Custom views must now provide reflection metadata
hidden: true
type: breaking
---

### Custom views must now provide reflection metadata

(This change only affects users of the Rust API for registering custom views.)

`App::add_view_class` now requires a `ViewReflection` argument describing which archetypes the view supports.

Before:

```rust
app.add_view_class::<MyView>()?;
```

After:

```rust
app.add_view_class::<MyView>(rerun::reflection::ViewReflection {
    applicability: rerun::reflection::ViewApplicability::Archetypes(vec![
        <rerun::archetypes::Points3D as rerun::Archetype>::name(),
    ]),
})?;
```

Use `ViewApplicability::AllArchetypes` for a custom view that is not dependent on any particular archetype.

This information may be used by the Viewer for various heuristics.
