---
title: Overview
order: 0
---
The following sections give an overview of the basic ui concepts and where to find which functionality.

Generally, the viewer tries to be as self-explaining as possible - most items in the ui show a tooltip upon hovering which should give additional information.
If you are missing a piece of information, don't hesitate to [file an issue](https://github.com/rerun-io/rerun/issues/new/choose)!

Overview
--------------------------
![screenshot of the viewer with different parts annotated](/docs-media/viewer-overview.png)

### [Blueprint](blueprint.md)
The Blueprint view is where you see and edit the Blueprint for the whole viewer, i.e. what is shown in the viewer (and how it is shown).

### [Selection](selection.md)
The Selection view let's you see details and edit configurations of the current selection(s).

### [Timeline](timeline.md)
The timeline panel gives you controls over what point in time you're looking at on which [timeline](../../concepts/timelines.md) for the rest of the viewer.
Additionally, it gives you an overview of all events on a given timeline.

### [Viewport](viewport.md)
The viewport is where your visualizations live. It is composed of one or more Space Views that you can arrange freely.

### Top bar & Menu
The top bar contains operating system controls and generic information.
In the menu you find application wide options and actions.
Use the buttons at the top right corner to hide/show parts of the viewer.

Command Palette
----------------------------
The command palette is a powerful tool to reach arbitrary actions from anywhere via a simple text search.
You reach it with `cmd/ctrl + P` or via the menu.
![screenshot of the command palette](/docs-media/command-palette.png)
Once it's open just start typing to filter and press `Enter` to execute the selected action or cancel with `Esc`.

[TODO(#1132)](https://github.com/rerun-io/rerun/issues/1132): The command palette is too limited right now.

Help icons
----------
Most views have an info icon at the top right corner.
![help icon](/docs-media/help-icon.png)
On hover it displays additional information on how to use a view.

Event Log
---------
By default, the viewer will show the main viewport in order to explore your data.
With the Event Log, the viewer offers a dedicated mode for exploring raw logged raw messages from your application which will hide the main viewport.

You switch back and forth between the Viewport and the Event Log with the main menu in the top bar:
![the event log and how to get there](/docs-media/event-log.png)
The Event Log shows all messages in a comprehensive table in the order they arrive in.
