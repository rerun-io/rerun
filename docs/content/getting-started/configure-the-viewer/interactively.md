---
title: Configure the Viewer interactively
order: 1
---

The Rerun Viewer is configurable directly through the UI itself.

## Viewer overview

<picture>
  <img src="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/1200w.png">
</picture>


The central part is known as the viewport and contains the various views displaying the data.

The left panel of the Viewer is the "Blueprint Panel". It shows a visual tree view representing
the contents of the current blueprint.

The right panel of the Viewer is the "Selection Panel" this panel allows you to configure
specific blueprint properties of the currently selected element.

The blueprint defines the structure, the type of views, and their content in the viewport. Changing the content of the viewport is done by editing the blueprint.

After editing the Viewer you may want to [save or share the blueprint](./save-and-load.md).

## Configuring the view hierarchy

The viewport is made of various views, laid out hierarchically with nested containers of various kinds: vertical, horizontal, grid, and tabs. This hierarchy is represented in the blueprint panel, with the top container corresponding to the viewport. In this section, we cover the various ways this view hierarchy can be interactively created and modified.

### Show or hide parts of the blueprint

Any container or view can be hidden or shown by clicking the "eye" icon.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/show_hide_btn/bbca385d4898ec220bfb91c430ea52d59553913e/full.png" alt="">
</picture>


### Add new containers or views

Adding a container or a view to the view port can be done by clicking the "+" at the top of the blueprint panel.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_view/3933d7096846594304ddec2d51dda9c434d763bf/full.png" alt="">
</picture>


If a container (or the viewport) is already selected, a "+" button will also be available in the selection panel.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_view_selection_panel/2daf01c80dcd2496b554e4376af702c7713a47dc/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/add_view_selection_panel/2daf01c80dcd2496b554e4376af702c7713a47dc/480w.png">
</picture>



### Remove a view or container

Removing a view or a container can be done by clicking the "-" button next to it:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/remove/6b9d97e4297738b8aad89158e4d15420be362b4a/full.png" alt="">
</picture>


### Re-arrange existing containers or views

The viewport hierarchy can be reorganized by drag-and-dropping containers or views in the blueprint panel. It ssi also possible to drag views directly in the viewport by using their title tab:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/drag_and_drop_viewport/8521fda375a2f6af15628b04ead4ba848cb8bc27/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/drag_and_drop_viewport/8521fda375a2f6af15628b04ead4ba848cb8bc27/480w.png">
</picture>


### Rename a view or container

Both views and containers may be assigned a custom name. This can be done by selecting the view or container, and editing the name at the top of the selection panel.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/rename/9dcb63d36f1676568fb106ee55ab110438b63fa9/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rename/9dcb63d36f1676568fb106ee55ab110438b63fa9/480w.png">
</picture>


### Change a container kind

Containers come in four different kinds: vertical, horizontal, grid, and tabs. To change an existing container's kind, select it and change the value from the dropdown menu in the selection panel:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/container_kind/f123f2220d9e82d520af367b7af020179a4de675/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/container_kind/f123f2220d9e82d520af367b7af020179a4de675/480w.png">
</picture>



### Using context menus

The context menu is accessed by right-clicking on a container or view in the blueprint panel. Many of the previous operations are also available there:

<picture>
  <img src="https://static.rerun.io/context_menu_container/e90e4688f306187d902467b452fb7146eec1bf4b/full.png" alt="">
</picture>


One key advantage of using the context menu is that it enable operations on multiple items at once. For example, you may select several views (ctrl-click, or cmd-click on Mac), and remove them all in a single operation using the context menu.


## Configuring the content of a view

The content of a view is determined by its entity query, which can be manually edited in the selection panel when the view is selected (see [Entity Queries](../../reference/entity-queries.md) for more information). This section covers the interactive means of manipulating the view content (which typically operate by actually modifying the query).


### Show or hide view content

Like containers and views, any entity in a view may be shown and hidden with the "eye" icon or the context menu.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/show_hide_entity/587a5d8fd763c0bade461bc54a66a4acdd087821/full.png" alt="">
</picture>


### Remove data from a view

Likewise, entities may be removed from a view by clicking the "-" next to it:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/remove_entity/ec0447ca7e420bc9d19a7bf015cc39f88b42598a/full.png" alt="">
</picture>


### Using the query editor

A visual query editor is available from the selection panel when a view is selected. Click the "Edit" button next to the entity query:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_remove_entity/4c5e536d4ca145058a8bc59a0b32267821663f06/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/add_remove_entity/4c5e536d4ca145058a8bc59a0b32267821663f06/480w.png">
</picture>

The query editor allows visually adding and removing entities and entity trees from the query.

### Adding entities to a new view with context menu

Like with viewport hierarchy, most operations on view data are available from the context menu. In particular, a new view can be created with custom content by selecting one or more entities (either in existing views in the blueprint panel, or in the time panel's streams), and clicking "Add to new space view" in the context menu:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/1024w.png">
</picture>

When using one of the recommended views with this method, the view's origin will automatically be set to a sensible default based on the actual data.

### Overriding visualizers and component values

By selecting an entity from a view, it is possible to specify which visualizer(s) should be enabled, and to override values of specific components:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/visualizers/8ca9926398435e8b4c46eda91267a5454f6a75ba/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/visualizers/8ca9926398435e8b4c46eda91267a5454f6a75ba/480w.png">
</picture>

When selecting a view, it is also possible to specify default values for components of a given type, which is used when no value has been logged:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component_defaults/3330f16b246a523f9e9a9d8c3549cdc08660356f/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component_defaults/3330f16b246a523f9e9a9d8c3549cdc08660356f/480w.png">
</picture>

See [Visualizers and Overrides](../../concepts/visualizers-and-overrides.md) for detailed information and more examples.

