---
title: Configure the Viewer through code (Blueprints)
order: 100
---

As of Rerun 0.15, the state of the [blueprint](../../reference/viewer/blueprint.md) can be directly manipulated using the
Rerun SDK.

In the initial 0.15 release, the APIs are still somewhat limited and only available in the Python SDK.
Future releases will add support for the full scope of blueprint. See issues: [#5519](https://github.com/rerun-io/rerun/issues/5519), [#5520](https://github.com/rerun-io/rerun/issues/5520), [#5521](https://github.com/rerun-io/rerun/issues/5521).

## Blueprint API overview

All blueprint APIs are in the [`rerun.blueprint`](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) namespace. In our Python examples, we typically import this using the `rrb` alias:

```python
import rerun.blueprint as rrb
```

The Python blueprint API is declarative and object-centric. There are 3 main types of blueprint objects you will
encounter:

-   `Blueprint`: The root object that represents the entire Viewer layout.
-   `Container`: A layout object that contains other containers or views.
-   `View`: A view object that represents a single view of the data.

Both containers and views should be used via typed subclasses instead.:

-   `Container` has subclasses: `Horizontal`, `Vertical`, `Grid`, and `Tabs`.
-   `View` has subclasses: `BarChartView`, `Spatial2DView`, `Spatial3DView`, `TensorView`,
    `TextDocumentView`, `TextLogView`, and `TimeSeriesView`.

These paths can be combined hierarchically to create a complex Viewer layout.

For example:

```python
my_blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.BarChartView(),
        rrb.Vertical(
            rrb.Spatial2DView(),
            rrb.Spatial3DView(),
        ),
    ),
)
```

## Sending the blueprint to the Viewer

To provide a blueprint, simply pass it to either `init` or `connect_grpc` using the `default_blueprint`
parameter.

Using `init` with `spawn=True`:

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint", spawn=True, default_blueprint=my_blueprint)
```

Or if you use `connect_grpc` separate from `init`:

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint")

...

rr.connect_grpc(default_blueprint=my_blueprint)
```

## Activating the default blueprint

Just like the Viewer can store many different recordings internally, it can also
store many different blueprints. For each `application_id` in the viewer, are two
particularly important blueprints: the "default blueprint" and the "active blueprint".

When a recording is selected, the active blueprint for the corresponding
`application_id` will completely determine what is displayed by the viewer.

When you send a blueprint to the viewer, it will not necessarily be
activated immediately. The standard behavior is to only update the "default
blueprint" in the viewer. This minimizes the chance that you accidentally
overwrite blueprint edits you may have made locally.

If you want to start using the new blueprint, after sending it, you will need to
click the reset button (<img src="https://static.rerun.io/b60eb3c4010e3ee46bbeeabf3da411fade2495b6_reset.png" alt="reset icon" style="display:inline; vertical-align: middle; height: 20px; margin: 0px"/>) in the blueprint panel. This resets the active blueprint to the
current default.

## Always activating the blueprint

If you want to always activate the blueprint as soon as it is received, you can instead use the `send_blueprint`
API. This API has two flags `make_active` and `make_default`, both of which default to `True`.

If `make_active` is set, the blueprint will be activated immediately. Exercise care in using this API, as it can be
surprising for users to have their blueprint changed without warning.

```python
my_blueprint = rrb.Blueprint(...)

rr.init("rerun_example_my_blueprint", spawn=True)

rr.send_blueprint(my_blueprint, make_active=True)

```

## Customizing views

Any of the views (`BarChartView`, `Spatial2DView`, `Spatial3DView`, `TensorView`,
`TextDocumentView`, `TextLogView`, or `TimeSeriesView`) can be instantiated with no arguments.
By default these views try to include all compatible entities.

For example, the following blueprint creates a single 3D view that includes all the 3D content
you have logged to the entity tree:

```python
rrb.Blueprint(
    rrb.Spatial3DView()
)
```

Beyond instantiating the views, there are 3 parameters you may want to specify: `name`, `origin`, and `contents`.

`name` is simply the name of the view used as a label in the viewer.

However, both `origin` and `contents` play an important role in determining what data is included in the view.

### `origin`

The `origin` of a view is a generalized "frame of reference" for the view. We think of showing all data
in the view as relative to the `origin`.

By default, only data that is under the `origin` will be included in the view. As such this is one of the most
convenient ways of restricting a view to a particular subtree.

Because the data in the view is relative to the `origin`, the `origin` will be the first entity displayed
in the blueprint tree, with all entities under the origin shown using relative paths.

For Spatial views such as `Spatial2DView` and `Spatial3DView`, the `origin` plays an additional role with respect
to data transforms. All data in the view will be transformed to the `origin` space before being displayed. See [Spaces and Transforms](../../concepts/spaces-and-transforms.md) for more information.

For example:

```python
rrb.Blueprint(
    rrb.Horizontal(
        rrb.Spatial3DView(origin="/world"),
        rrb.Spatial2DView(origin="/world/robot/camera"),
    )
)
```

### `contents`

If you need to further modify the contents of a view, you can use the `contents` parameter. This parameter is
a list of [entity query expressions](../../reference/) that are either included or excluded from the
view.

Each entity expressions starts with "+" for inclusion or "-" for an exclusion. The expressions can either be specific entity paths, or may end in a wildcard `/**` to include all entities under a specific subtree.

When combining multiple expressions, the "most specific" rule wins.

Additionally, these expressions can reference `$origin` to refer to the origin of the view.

For example:

```python
rrb.Blueprint(
    rrb.Horizontal(
        rrb.Spatial3DView(
            origin="/world",
            contents=[
                "+ $origin/robot/**",
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

## Implicit conversion

For convenience all of the blueprint APIs take a `BlueprintLike` rather than requiring a `Blueprint` object.
Both `View`s and `Containers` are considered `BlueprintLike`. Similarly, the `Blueprint` object can
take a `View` or `Container` as an argument.

All of the following are equivalent:

```python
rr.send_blueprint(rrb.Spatial3DView())
```

```python
rr.send_blueprint(
    rrb.Grid(
        Spatial3DView(),
    )
)
```

```python
rr.send_blueprint(
    rrb.Blueprint(
        Spatial3DView(),
    ),
)

```

```python
rr.send_blueprint(
    rrb.Blueprint(
        rrb.Grid(
            Spatial3DView(),
        )
    ),
)
```

## Customizing the top-level blueprint

The top-level `Blueprint` object can also be customized.

### Controlling the panel state

The `Blueprint` controls the default panel-state of the 3 panels: the `BlueprintPanel`, the `SelectionPanel`, and the `TimePanel`. These can be controlled by passing them as additional arguments to the `Blueprint` constructor.

```python
rrb.Blueprint(
    rrb.TimePanel(state="collapsed")
)
```

As an convenience, you can also use the blueprint argument: `collapse_panels=True` as a short-hand for:

```python
rrb.Blueprint(
    rrb.TimePanel(state="collapsed"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.BlueprintPanel(state="collapsed"),
)
```

### Controlling the auto behaviors

The blueprint has two additional parameters that influence the behavior of the viewer:

-   `auto_views` controls whether the Viewer will automatically create views for entities that are not explicitly included in the blueprint.
-   `auto_layout` controls whether the Viewer should automatically layout the containers when introducing new views.

If you pass in your own `View` or `Container` objects, these will both default to `False` so that the Blueprint
you get is exactly what you specify. Otherwise they will default to `True` so that you will still get content (this
matches the default behavior of the Viewer if no blueprint is provided).

This means that:

```python
rrb.Blueprint()
```

and

```python
rrb.Blueprint(
    auto_views=True,
    auto_layout=True
)
```

are both equivalent to the viewer's default behavior.

If you truly want to create an empty blueprint, you must set both values to `False`:

```python
rrb.Blueprint(
    auto_views=False,
    auto_layout=False
),
```
