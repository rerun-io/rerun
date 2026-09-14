# Rerun's crates

Each folder is a layer.
A crate may depend on crates in its own folder, or in any folder below it in this list — never above.
`dev-dependencies` are exempt, since a test may reach anywhere.

[`../ARCHITECTURE.md`](../ARCHITECTURE.md) draws the layers as a diagram, and describes every crate in a table per folder.

## Where does my new crate go?

Pick the lowest folder whose description fits.
If the crate then needs something from a folder above it, either it belongs higher, or the thing it needs belongs lower.

1. [`tests`](tests) — exists only to serve tests. Nothing but a `dev-dependency` may point at it.
2. [`top`](top) — what our users depend on: the SDKs, the C and Python bindings, the CLI, and `re_viewer` — the app that hosts the views and the panels.
3. [`views`](views) and [`panels`](panels) — *siblings*. A view is a visualization a user puts in the viewport; a panel is a part of the app around it, or a widget one is built out of. Neither may depend on the other.
4. [`viewer_support`](viewer_support) — UI and rendering machinery that any view or panel may use: widgets, the renderer, viewer state. Knows about egui, but not about a specific view.
5. [`store_app`](store_app) — the queryable state a viewer or a server works with: entity databases, query engines, the in-memory server.
6. [`data_flow`](data_flow) — moving data in and out of Rerun: gRPC clients and servers, and readers for the file formats we import.
7. [`store`](store) — the data model and the store that holds it: chunks, components, encodings, and the protobuf types they travel as.
8. [`build`](build) — runs at build time only, and is never linked into anything we ship.
9. [`utils`](utils) — a small helper, not tied to Rerun's data model, that depends on no Rerun crate outside this folder.

Two folders on the same line are siblings: independent of each other, so neither may depend on the other in either direction.
`ARCHITECTURE.md` draws them side by side for that reason.

Adding a whole new layer means editing [`../scripts/check_crate_layers.py`](../scripts/check_crate_layers.py), [`../scripts/generate_crate_graph.py`](../scripts/generate_crate_graph.py), and the `members` list in [`../Cargo.toml`](../Cargo.toml).

## Enforcement

[`../scripts/check_crate_layers.py`](../scripts/check_crate_layers.py) fails in CI on a dependency that points up, and [`../ARCHITECTURE.md`](../ARCHITECTURE.md) draws the layering, one band per folder.

To fix a dependency that points the wrong way, move a crate — do not reorder the layers.
