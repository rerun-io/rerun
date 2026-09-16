# re_renderer_tests

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

Snapshot tests for `re_renderer`'s rendering primitives, with `epaint` rendering the same
geometry next to it as a visual baseline.

These tests do not live in `re_renderer` itself because they need `egui_kittest` and
`re_test_context`, both of which sit above `re_renderer` in the dependency graph.
