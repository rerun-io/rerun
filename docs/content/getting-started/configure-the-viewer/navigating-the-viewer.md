---
title: Navigating the Viewer
order: 400
---

This page will walk you through the basics of navigating the Rerun Viewer.

By default, the Rerun Viewer uses heuristics to automatically determine an appropriate layout for your data. However, you'll often want precise control over how your data is displayed. Blueprints give you complete control over the Viewer's layout and configuration. For a conceptual understanding of blueprints, see [Blueprints](../../concepts/visualization/blueprints.md).

This guide covers three complementary ways to work with the viewer:
- **[Interactive configuration](#interactive-configuration)**: Modify layouts directly in the Viewer UI
- **[Save and load blueprint files](#save-and-load-blueprint-files)**: Share layouts using `.rbl` files
- **[Programmatic blueprints](#programmatic-blueprints)**: Control layouts from code

## Interactive configuration

The Rerun Viewer is fully configurable through its UI, making it easy to experiment with different layouts.

### Viewer overview

<picture>
  <img src="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/overview/158a13691fe0364ed5d4dc420f5b2c39b60705cd/1200w.png">
</picture>

The Viewer consists of:
- **Viewport** (center): Contains your views, arranged in containers
- **Blueprint Panel** (left): Shows the visual tree of your blueprint structure
- **Selection Panel** (right): Displays properties of the selected element
- **Time Panel** (bottom): Controls timeline playback and navigation

The blueprint defines what appears in the viewport. All changes you make to the viewport are actually changes to the blueprint.

### Configuring the view hierarchy

The viewport contains views arranged hierarchically using containers. Containers come in four types:
- **Horizontal**: Arranges views side-by-side
- **Vertical**: Stacks views top-to-bottom
- **Grid**: Organizes views in a grid layout
- **Tabs**: Shows views in tabs (only one visible at a time)

#### Add new containers or views

Click the "+" button at the top of the blueprint panel to add containers or views.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_view/3933d7096846594304ddec2d51dda9c434d763bf/full.png" alt="">
</picture>

If a container (or the viewport) is selected, a "+" button also appears in the selection panel.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_view_selection_panel/2daf01c80dcd2496b554e4376af702c7713a47dc/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/add_view_selection_panel/2daf01c80dcd2496b554e4376af702c7713a47dc/480w.png">
</picture>

#### Rearrange views and containers

Drag and drop items in the blueprint panel to reorganize the hierarchy. You can also drag views directly in the viewport using their title tabs.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/drag_and_drop_viewport/8521fda375a2f6af15628b04ead4ba848cb8bc27/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/drag_and_drop_viewport/8521fda375a2f6af15628b04ead4ba848cb8bc27/480w.png">
</picture>

#### Show, hide, or remove elements

Use the eye icon to show or hide any container, view, or entity:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/show_hide_btn/bbca385d4898ec220bfb91c430ea52d59553913e/full.png" alt="">
</picture>

Use the "-" button to permanently remove an element:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/remove/6b9d97e4297738b8aad89158e4d15420be362b4a/full.png" alt="">
</picture>

#### Rename views and containers

Select a view or container and edit its name at the top of the selection panel.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/rename/9dcb63d36f1676568fb106ee55ab110438b63fa9/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rename/9dcb63d36f1676568fb106ee55ab110438b63fa9/480w.png">
</picture>

#### Change container type

Select a container and change its type using the dropdown in the selection panel.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/container_kind/f123f2220d9e82d520af367b7af020179a4de675/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/container_kind/f123f2220d9e82d520af367b7af020179a4de675/480w.png">
</picture>

#### Using context menus

Right-click on any element in the blueprint panel for quick access to common operations:

<picture>
  <img src="https://static.rerun.io/context_menu_container/e90e4688f306187d902467b452fb7146eec1bf4b/full.png" alt="">
</picture>

Context menus support multi-selection (Ctrl+click or Cmd+click), enabling bulk operations like removing multiple views at once.

### Configuring view content

Each view displays data based on its entity query. You can modify what appears in a view interactively.

#### Show or hide entities

Use the eye icon next to any entity to control its visibility.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/show_hide_entity/587a5d8fd763c0bade461bc54a66a4acdd087821/full.png" alt="">
</picture>

#### Remove entities from views

Click the "-" button next to an entity to remove it from the view.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/remove_entity/ec0447ca7e420bc9d19a7bf015cc39f88b42598a/full.png" alt="">
</picture>

#### Using the query editor

With a view selected, click "Edit" next to the entity query in the selection panel to visually add or remove entities.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_remove_entity/4c5e536d4ca145058a8bc59a0b32267821663f06/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/add_remove_entity/4c5e536d4ca145058a8bc59a0b32267821663f06/480w.png">
</picture>

#### Creating views from entities

Select one or more entities (in existing views or in the time panel's streams), right-click, and choose "Add to new view" from the context menu.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/add_to_new_view/87f2d5ffb3ef896c82f398cd3c3d1c7321d59073/1024w.png">
</picture>

The view's origin will automatically be set based on the selected data.

### Overriding visualizers and components

Select an entity within a view to control which visualizers are used and override component values.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/visualizers/826c026c9e26b5fa4f899214f488f13d363816fc/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/visualizers/826c026c9e26b5fa4f899214f488f13d363816fc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/visualizers/826c026c9e26b5fa4f899214f488f13d363816fc/768w.png">
</picture>

When selecting a view, you can also set default component values that apply when no value has been logged.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component_defaults/4c0e3ea9d0aa3cbc0eb2f0c444b4a58a765a674d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component_defaults/4c0e3ea9d0aa3cbc0eb2f0c444b4a58a765a674d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component_defaults/4c0e3ea9d0aa3cbc0eb2f0c444b4a58a765a674d/768w.png">
</picture>

See [Visualizers and Overrides](../../concepts/visualization/visualizers-and-overrides.md) for detailed information.

---

## Save and load blueprint files

Once you've configured your layout, you can save it as a blueprint file (`.rbl`) to reuse across sessions or share with your team.

### Saving a blueprint

To save your current blueprint, go to the file menu and choose "Save blueprint…":

<picture>
  <img src="https://static.rerun.io/save_blueprint/85644e086ba9cf7fb81cb7ece55b38bef863c755/full.png" alt="">
</picture>

Blueprint files are small, portable, and can be version-controlled alongside your code.

### Loading a blueprint

Load a blueprint file using "Open…" from the file menu, or simply drag and drop the `.rbl` file into the Viewer.

**Important:** The blueprint's Application ID must match the Application ID of your recording. Blueprints are bound to specific Application IDs to ensure they work with compatible data structures. See [Application IDs](../../concepts/visualization/blueprints.md#application-ids-binding-blueprints-to-data) for more details.

### Sharing blueprints

Blueprint files make it easy to ensure everyone on your team views data consistently:

1. Configure your ideal layout interactively
2. Save the blueprint to a `.rbl` file
3. Commit the file to your repository
4. Team members load the blueprint when viewing recordings with the same Application ID

This is particularly valuable for:
- **Debugging sessions**: Share the exact layout needed to diagnose specific issues
- **Presentations**: Ensure consistent visualization across demos
- **Data analysis**: Standardize views for comparing results

---

## Next steps

- **Explore view types**: Check the [View Type Reference](../../reference/types/views/) to see all available views and their configuration options
- **Learn about overrides**: See [Visualizers and Overrides](../../concepts/visualization/visualizers-and-overrides.md) for per-entity customization
- **API Reference**: Browse the complete [Blueprint API](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) for programmatic control
