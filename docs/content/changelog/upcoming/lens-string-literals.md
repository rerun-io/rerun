---
title: String literals in lens selectors
hidden: true
type: feature
---

### String literals in lens selectors

[Lens selectors](../concepts/query-and-transform/lenses.md) now support string literals such as `"foo"` to emit one constant string per input value.
For example, `.location.x | "foo"` emits `"foo"` once per selected value, without a custom function.
