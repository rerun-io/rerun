---
title: Blueprint APIs
order: 2
---

As of Rerun 0.15, the state of the [Blueprint](../../concepts/blueprint.md) can be directly manipulated using
"Blueprint APIs" in the SDK.

In the initial 0.15 Release, the APIs are still somewhat limited and only available in the Python SDK.
Future releases will add support for the full scope of Blueprint. See issues: [#5519](https://github.com/rerun-io/rerun/issues/5519), [#5520](https://github.com/rerun-io/rerun/issues/5520), [#5521](https://github.com/rerun-io/rerun/issues/5521).

## Blueprint API Overview

All blueprint APIs are in the `rerun.blueprint` namespace. In our python examples, we typically import this using the `rrb` alias:

```python
import rerun.blueprint as rrb
```

The python blueprint API is declarative and object-centric. There are 3 main types of blueprint objects you will
encounter:

-   `Blueprint`: The root object that represents the entire viewer layout.
-   `Container`: A layout object that contains other containers or views.
-   `SpaceView`: A view object that represents a single view of the data.

Both containers and spaceviews should be used via typed subclasses instead.:

-   `Container` has subclasses: `Horizontal`, `Vertical`, `Grid`, and `Tabs`.
-   `SpaceView` has subclasses: `BarChartView`, `Spatial2DView`, `Spatial3DView`, `TensorView`,
    `TextDocumentView`, `TextLogView`, and `TimeSeriesView`.

These paths can be combined hierarchically to create a complex viewer layout.

For example:

```python
my_blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.SpaceView(rrb.BarChartView()),
        rrb.Vertical(
            rrb.SpaceView(rrb.Spatial2DView()),
            rrb.SpaceView(rrb.Spatial3DView()),
        ),
    ),
)
```

## Sending the Blueprint to the Viewer

To use a blueprint, simply pass it to either `init` or `connect`:

If you use `init` with the `spawn=True` option, you should pass the blueprint as an argument:

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint", spawn=True, blueprint=my_blueprint)
```

Or if you use `connect` separate from `init`, you should pass blueprint when you call `connect`:

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint")

...

rr.connect(blueprint=my_blueprint)
```

## Customizing Space Views

If you create a space view with no arguments, by default it will try to include all compatible entities
in the entire tree.

There are 3 parameters you may want to specify for a space view: `name`, `origin`, and `contents`.

`name` is simply the name of the view used as a label in the viewer.

However, both `origin` and `contents` play an important role in determining what data is included in the view.

### `origin`

TODO(jleibs): this explanation probably belongs somewhere else?

The `origin` of a space-view is a generalized "frame of reference" for the view. We think of showing all data
in the space view as relative to the `origin`.

By default, only data that is under the `origin` will be included in the view. As such this is one of the most
convenient ways of restricting a space-view to a particular subtree.

Because the data in the space-view is relative to the `origin`, the `origin` will be the first entity displayed
in the blueprint tree, with all entities under the origin shown using relative paths.

For Spatial views such as `Spatial2DView` and `Spatial3DView`, the `origin` plays an additional role with respect
to data transforms. All data in the view will be transformed to the `origin` space before being displayed. See [Spaces and Transforms](../../concepts/spaces-and-transforms.md) for more information.
TODO(jleibs): Re-review spaces-and-transforms for correctness

For example:

```python
my_blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.Spatial3DView(origin="/world"),
        rrb.Spatial2DView(origin="/world/robot/camera"),
    )
)
```

### `contents`

If you need to further modify the contents of a space view, you can use the `contents` parameter. This parameter is
a list of [entity query expressions](../../concepts/entity-queries.md) that are either included or excluded from the
view.

Each entity expressions starts with "+" for inclusion or "-" for an exclusion. The expressions can either be specific entity paths, or may end in a wildcard `/**` to include all entities under a specific subtree.

When combining multiple expressions, the "most specific" rule wins.

Additionally, these expressions can reference `$origin` to refer to the origin of the space view.

For example:

```python
my_blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.Spatial3DView(
            origin="/world",
            contents=[
                "+ $origin/robot/**",
                "- $origin/map/**",
            ],
        ),
        rrb.Spatial2DView(
            origin="/world/robot/camera",
            contents=[
                "+ $origin/**",
                "+ /world/robot/actuator/**",
            ],
        ),
    )
)

```
