---
title: Overview
order: 100
description: The panels of the Rerun Viewer and where to find each piece of functionality
---
The following sections give an overview of the basic UI concepts and where to find which functionality.

Most items in the UI show a tooltip with more information when you hover them.
If information is missing, [file an issue](https://github.com/rerun-io/rerun/issues/new/choose).

Overview
--------------------------
![screenshot of the Viewer with different parts annotated](https://static.rerun.io/a5e708e4bbd2c0b182f7f9103ab42c85e55f8982_viewer-overview.png)

### [Blueprint](blueprint-panel.md)
The blueprint panel is where you see and edit the blueprint for the whole Viewer, i.e. what is shown in the Viewer (and how it is shown).

### [Selection](selection.md)
The selection panel lets you see details and edit configurations of the current selection(s).

### [Timeline](timeline.md)
The timeline panel gives you controls over what point in time you're looking at on which [timeline](../../logging-and-ingestion/data-model/timelines.md) for the rest of the viewer.
Additionally, it gives you an overview of all events on a given timeline.

### [Viewport](viewport.md)
The viewport is where your visualizations live. It is composed of one or more Views that you can arrange freely.

### Top bar & menu
The top bar contains operating system controls and generic information.
In the menu you find application wide options and actions.
Use the buttons at the top right corner to hide/show parts of the viewer.

Command Palette
----------------------------
The command palette lets you run any action from anywhere by searching for it by name.
You reach it with `cmd/ctrl + K` or via the menu.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/command-palette/76d89ff6d2b768c718c84462c6e2bdaa54e40e54/480w.png">
  <img src="https://static.rerun.io/command-palette/76d89ff6d2b768c718c84462c6e2bdaa54e40e54/full.png" alt="screenshot of the command palette">
</picture>


Once it's open just start typing to filter and press `Enter` to execute the selected action or cancel with `Esc`.

The command palette does not yet cover every UI action ([#1132](https://github.com/rerun-io/rerun/issues/1132)).
<!-- TODO(#1132): update this when the command palette covers all UI actions. -->

Help icons
----------
Most views have an info icon at the top right corner.

<picture>
  <img src="https://static.rerun.io/help-icon/d6268a4576bad594b0c29cf77881d7f1bb9bb889/full.png" alt="help icon">
</picture>


On hover it displays additional information on how to use a view.
