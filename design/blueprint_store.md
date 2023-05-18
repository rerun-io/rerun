# Blueprint store
Status: pre-proposal

## Intro
How do we store our UI state? We want:

- To be able to set up the UI from logging SDK
- Save the UI state to disk
- Undo/redo

UI state include:

- Is the selection panel open? how wide?
- How are my space views organized?
- What data is shown in each space view

## Proposal

We have a separate “blueprint store”, implemented with the same DataStore code. We will therefore have both “data entities” and “blueprint entities”.

Each *view* has a unique `BlueprintId`. We have a few built-in `BlueprintId`s:

- `viewport`, `selection`, `stream`, …
- Then auto-generated uuids for each other space view, data group, layout, data blueprint, etc

The blueprint store is organized flatly by just blueprint id, e.g. `viewport.children = […]` where `children` is a *ui component (or “blueprint component”?). In this case `children` contain other blueprint ids (the views in the viewport).

The blueprint store has exactly one timeline: `ui_time` which is the local time of the application. This can then be used for undo and redo.

## Viewer
The viewer state would be completely driven by the blueprint store and the data store. Each frame we would basically query the blueprint store about the current state of the ui, which would then dictate the queries done to the data store.

The user save the data store and blueprint store to separate files.

### Python SDK
Something like this is very low-level, and not very ergonomic:

```py
viewer.ui_log("top_bar", [("visible", false)])
space_view_bpid = viewer.ui_new_space_view("My 3D view")
data_bpid = viewer.new_data_blueprint("world/points")
viewer.ui_log(space_view_bpid, [("category", "3d"), ("children", [data_bpid])])
viewer.ui_log("viewport", [("children", [space_view_bpid])])
```

Adding high-level helpers on top of this is very desirable, but a lot of work.


### UI components
The ui components are quite specific for the type of blueprint. Here are a few example:

* `root`:
    * `TimeControl` (globally selected timeline, time, play state, etc)
* Top bar:
    * `visible`
* Blueprint panel:
    * `visible`
    * `width`
* Selection panel:
    * `visible`
    * `width`
* Time panel:
    * `visibility` ("hidden", "collapsed", "expanded")
* Viewport:
    * `children` (max one!)
* Layout
    * `children`
    * `type`: "horizontal", "vertical", "auto", …
    * `sizes`: individual sizes of the children
* Space view
    * `children` (data blueprints)
    * `category` ("3d", "text", …)
* Data group
    * `children`
* Data
    * `entity_path` data entity path

To help make the transition easy we should consider creating a shim between `arrow2` and `serde`.


## Future work
We support data overrides and defaults using:

* `blueprint_id/default/$data_entity_path.component`
* `blueprint_id/override/$data_entity_path.component`

Can `$data_entity_path` be a pattern, or just a full path?

Example, default point size for everything in the viewport: `viewport/default/**.radius = 2.0pt`
