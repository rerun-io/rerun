---
title: "More flexible experimental table blueprints"
hidden: true
type: feature
---

### More flexible experimental table blueprints

Table configuration was previously quite ad hoc. With the advent of complex previews in tables,
we've already started adding some blueprint support for tables and are now leaning into it more and more!
Experimental table blueprints can now:

- Define separate table and card layouts.
- Choose the default layout.
- Order, rename, and hide columns.
- Select card titles and links.
- Configure how each column is displayed, including live recording previews.
- Use multiple preview columns.
- Use editable boolean flag columns in non-card layouts.
- Configure the timeline for recording previews.

These features are currently only available through very low-level blueprint archetypes.
A clean Python API and more configuration options will follow soon!
<!-- TODO(RR-4810): when adding nice bp api mention that here -->

Example: https://github.com/rerun-io/rerun/blob/latest/examples/python/table_blueprints
