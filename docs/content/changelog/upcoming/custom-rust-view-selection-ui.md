---
title: Breaking changes to custom Rust views
hidden: true
type: breaking
---

### Breaking changes to custom Rust views

These changes only affect the Rust API for custom views.

#### Property reflection metadata

`ViewReflection` now requires a `property_archetypes` field describing the view's property archetypes in display order.
Use `property_archetypes: vec![]` for views without properties.
Listed property archetypes must have reflection metadata available to the Viewer.

#### Selection-panel UI

Previously, `ViewClass::selection_ui` rendered directly into the fixed "View properties" section, while `ViewClass::visualizers_section` optionally supplied a separate visualizers section.
Custom views could not hide the standard entity-path filter or add their own top-level sections through these APIs.

`ViewClass::selection_ui` now takes a `ViewContext` and returns a `ViewSelectionUi` configuration instead of rendering directly and returning a `Result`.
Custom views can now:

- Hide the standard entity-path filter with `show_entity_filter`.
- Configure the visualizers section and its add-visualizer menu through `visualizers`.
- Insert titled, collapsible `extra_sections` between the visualizers and view properties.
- Replace the "View properties" contents with a `blueprint_properties` callback, or leave it unset to render the reflected property archetypes automatically.

Views that only listed properties by hand can omit this method once those properties are declared in their reflection metadata.
To replace the automatic property UI, move custom UI into a callback:

```rust
fn selection_ui<'a>(
    &'a self,
    _ctx: &re_viewer_context::ViewContext<'_>,
) -> re_viewer_context::ViewSelectionUi<'a> {
    re_viewer_context::ViewSelectionUi::properties_ui(|ui, ctx| {
        ui.label(format!("View: {:?}", ctx.view_id));
        Ok(())
    })
}
```

The callback's `ctx.view_state` is shared rather than mutable; state changes require interior mutability or queued actions for the view's mutable update/render path.
To customize individual property fields, render the remaining fields with `re_view::view_property_ui_with_hidden_components` and the custom fields with `re_view::view_property_component_ui_custom` inside the callback.

`ViewClass::visualizers_section` has been removed; return its output through `ViewSelectionUi::visualizers` instead.

**This API remains unstable and will continue to evolve.**
Expect further breaking changes as we refine selection-panel customization for custom views.
