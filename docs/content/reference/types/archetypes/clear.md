---
title: "Clear"
---

Empties all the components of an entity.

The presence of a clear means that a latest-at query of components at a given path(s)
will not return any components that were logged at those paths before the clear.
Any logged components after the clear are unaffected by the clear.

This implies that a range query that includes time points that are before the clear,
still returns all components at the given path(s).
Meaning that in practice clears are ineffective when making use of visible time ranges.
Scalar plots are an exception: they track clears and use them to represent holes in the
data (i.e. discontinuous lines).

## Components

**Required**: [`ClearIsRecursive`](../components/clear_is_recursive.md)

## Links
 * 🌊 [C++ API docs for `Clear`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Clear.html)
 * 🐍 [Python API docs for `Clear`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.Clear)
 * 🦀 [Rust API docs for `Clear`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Clear.html)

## Examples

### Flat

code-example: clear_simple

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/1200w.png">
  <img src="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/full.png" width="640">
</picture>
</center>

### Recursive

code-example: clear_recursive

