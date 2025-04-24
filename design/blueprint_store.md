# Blueprint operation: APIs and datastore
Status: proposal

## Intro
The Blueprint APIs introduce a new mechanism of working with Rerun. While the Rerun data-logging APIs emphasize
low-friction capture of time-varying user-data *without* consideration of display or UI, the Blueprint APIs function to
give users explicit control over the details of how data is displayed in the viewer.

### How does a user describe the UI state programmatically?

 - Need to support complex and hierarchical layouts
 - Verbosity should not get in the way of simple use cases

### How do we store our UI state? We want:

- To be able to set up the UI from logging SDK
- Save the UI state to disk
- Support Undo/redo
- Also enable configuration of future UI plugins.

### UI state includes:

- Is the selection panel open? how wide?
- How are my views organized?
- What data is shown in each view
- Additional configuration / overrides for the data within each view

## Proposal

### Blueprint lifecycle
In order to simplify many edge cases, custom blueprints will only be able to be sent to the Viewer in their entirety as
part of Viewer startup. This limits blueprint control to: `rr.spawn()` (launch a native app), `rr.serve()` (launch a
hosted web-app), and `rr.show()` (embed a Viewer in a notebook). Additionally a blueprint file will be able to be
provided to the Viewer via the CLI at launch, or opened via the file-menu.

Blueprints will not otherwise be able to be sent via `rr.connect()`, which is reserved for only transmitting log-data to
an existing live Viewer instance, where a relevant blueprint is assumed to be loaded.

### Blueprint APIs

The blueprint APIs will follow a declarative structure, allowing users to build up a hierarchical blueprint object
that matches the Rerun concept map:
```yaml
App:
    Viewport:
        Container:
            SpaceView:
                DataGroup:
                    Data
                    Data
                    …
            Container:
                SpaceView:
                    DataGroup:
                        Data
                        Data
                        …
                SpaceView:
                    DataGroup:
                        Data
                        Data
                        …
            …
```

A theoretical Python API might look like:
```python
blueprint = rrb.App(
    expand_panels=False,
    time_control=rrb.TimeControl(
        timeline="sim_time",
        play_state="paused",
    ),
    viewport=rrb.VerticalLayout(
        content=[
            rrb.View3D(
                root="world",
                content=[
                    rrb.Data("./points", visible_history=10).override(radius=0.1, color="blue"),
                    rrb.Data("./camera").override(image_plane=3),
                ],
            ),
            rrb.HorizontalLayout(
                content=[
                    rrb.View2D(
                        root="world/camera/image",
                        content=rrb.DataGroup(
                            rrb.Data("."),
                            rrb.Points2D("world/points"),
                        ).default(radius=0.2),
                    ),
                    rrb.ViewTimeSeries("metrics", content=rrb.RecursiveData(".")),
                ],
            ),
        ],
    ),
)

rr.spawn(blueprint)
```

The assorted objects used in blueprint construction are:
 - `App`: Container for top-level application state such as panel-visibility, menus, etc.
 - `TimeControl`: Specific state relevant to the time controls.
 - `View`: Common base-class between `rrb.Container` and `rrb.SpaceView`
    - `Container`: A view that specifies layout of sub-views (interface only)
        - `HorizontalLayout`
        - `VerticalLayout`
        - … additional layouts
    - `SpaceView`: An actual view of data in a coordinate space.
        - `View2D`
        - `View3D`
        - `ViewTimeSeries`
        - … additional views
 - `Data`: A query that builds archetypes to draw in the view
    - `Auto`: A query to automatically build archetypes from an entity path
    - `Points2D`: A query to build a Points2D archetype
    - `Points3D`: A query to build a Points3D archetype
    - `Image`: A query to build an Image archetype
    - … additional typed archetype queries
 - `DataGroup`: A group of archetype queries with potentially shared overrides or defaults.
    - `RecursiveAuto`: A special DataGroup that recursively includes `Auto` queries for all entities under a given path.

Many Blueprint objects will allow for flexible upcasting into an obvious parent-type to reduce unnecessary typing. In
particular:
 - `Data` -> `DataGroup`
 - `[Data]` -> `DataGroup`
 - `DataGroup` -> `SpaceView` (If view category can be inferred)
 - `View` -> `Viewport`
 - `Viewport` -> `App`

This means a trivial expression like: `rr.show(rrb.Points3D("points"))` is still a valid Blueprint.

## Blueprint-Static data

As a further simplification, the Blueprint will allow for the direct inclusion of static data, allowing users to bypass
the data-logging APIs entirely for simple use-cases that don't require temporal information. This will be accomplished
by allowing `rrb.Data` objects to be constructed from any Rerun-loggable object.

Data that is a *query* from the recording store references an entity path used separately by the logging APIs:
```python
# Log data
for t in range(100):
    rr.set_time('step', sequence=t)
    rr.log("world/points", rr.Points3D(points))
…
# Construct blueprint
rrb.Auto("/world/points")
```
While static data skips the logging step all together, but only allows for a single element:
```python
rrb.Data(rr.Points3D(points))
```

This lets a user do things like display a grid of images:
```python
grid = rrb.GridLayout(cols=3)
for img in images:
    grid.appendView(rrb.View2D(root=None, content=rrb.Data(rr.Image(img))) for img in images)
```

Or more concisely using the implied upcasting rules:
```python
grid = rrd.GridLayout(cols=3, [rr.Image(img) for img in images])
```
Note the usage of `rr.Image` (the loggable) vs `rrb.Image` (the blueprint template).


## Blueprint store

Behind the APIs, the blueprint is implemented using a “blueprint store” that leverages the same code as the existing
data-store. We will therefore have both “data entities” and “blueprint entities”.

Before transmitting the Blueprint to the viewer, it will be serialized by emitting a sequence of blueprint entities
into a blueprint stream that can be loaded into the viewer.

Each piece of the blueprint has a unique `BlueprintId` which maps to an entity path in the Blueprint Store. Many of the
entities within the blueprint simply contain references to other blueprint-ids.

There is a reserved `BlueprintId`: "root", which is always the entry-point for the blueprint logic.
Most other types use auto-generated uuids as their BlueprintId.

For example:
```
/root
    .viewport: BlueprintId("/containers/ab33980a")
/containers
    /ab33980a
        .layout_class: LayoutClass::Horizontal
        .contents: [BlueprintId("/space_views/e514aac1"), BlueprintId("/space_views/e9f36821")]
        .shares: [2, 1]
/space_views
    /e514aac1
        .space_view_class: SpaceViewClass::View3D
        .eye: View3d::Eye(…)
        .contents: [BlueprintId("/data_groups/b117f5b9"), BlueprintId("/data_groups/8ee750a4")]
    /e9f36821
        …
/data_group
    /b117f5b9
        .contents: [EntityPath(RecordingStore, "/world/points")]
        /overrides
            .radius: 0.1
        …
    /8ee750a4
        .contents: [EntityPath(BlueprintStore, "/static/7db713c0")]
/static
    /7db713c0
        .positions: […]
        .colors: […]
```

Note that this means the blueprint store is mostly organized flatly with the hierarchy being represented by
references to other entities. It is considered an error to create circular references.

Because the blueprint store is just another data-store, inclusion of static data is fairly trivial. The data
is simply stored at an "anonymous" entity path within the blueprint store and will always be logged as Timeless.

The blueprint store has exactly one timeline: `ui_time` which is the local time of the application. This can then be used for undo and redo.

## Viewer
Any configurable Viewer state will be driven by the blueprint store and the data store. Each frame we will
query the blueprint store about the current state of the blueprint, which will then drive the layout of the UI.
In turn any user-interactions that modify the layout will be saved back to the blueprint store and queried again
on the next frame.

### UI components
The UI components are quite specific for the type of blueprint. Here are a few example:

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
* View
    * `children` (data blueprints)
    * `category` ("3D", "text", …)
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
