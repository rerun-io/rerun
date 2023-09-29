---
title: Loggable Data Types
order: 2
---

Rerun comes with built-in support for a number of different types that can be logged via the Python and Rust Logging
APIs and then visualized in the [Viewer](viewer.md).

The top-level types are called **archetypes** to differentiate them from the lower-level **data types** that make up the
individual components.  For more information on the relationship between **archetypes** and **components**, check out
the concept page on [Entities and Components](../concepts/entity-component.md).

In [Python](https://ref.rerun.io) every **archetype** is typically backed by one or more function calls. In
contrast, the [Rust API](https://docs.rs/rerun/) works by building up entities of a given archetype explicitly by
assembling the required components.

