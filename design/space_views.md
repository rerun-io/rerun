# Space Views
Status: Mostly implemented.


## What are Space Views
Space Views visualize a Data Blueprint, i.e. a set of entities with given properties.
They are represented as freely arrangeable tiles in the Viewport.
Most Space Views are interactive, allowing their data to be explored freely.


## Properties of a Space View
All properties are saved as part of the blueprint.

Immutable:
* Space View Class
* root entity path

Mutable:
* name
* positioning within layout
* class specific properties
* Data Blueprint

## Role of the root entity path
* root of the transform hierarchy (if any is used)
* may govern heuristics
* available at various stages of scene build-up (see below)

## Space View Class
Each Space View refers to an immutable Space View Class, implemented by `SpaceViewClass`.
It defines:
* which data it can display and how it is displayed
* how it is interacted with
* what properties are read from the blueprint store and how they are exposed in the ui

### What Space View Classes are there?
Space View differ only in class when they are **fundamentally different** in the way they display data.
Naturally, this means that there are only ever very few distinct Space View classes.

As of writing we have:
* Spatial
* Bar Chart
* Tensor
* Text
* Text Box
* Time Series

Proposal for future builtin, plus some notes:
* Spatial 2D
  * encompasses Tensor and Bar Chart
  * can display everything Spatial 3D can display
* Spatial 3D
  * can display everything Spatial 2D can display
* Data Table
  * New "raw" data view
* Text Log
  * Rename from Text
* RichText
  * Replaces text box
  * Supports markdown etc.
* Time Series
  * Need to figure out how this is constrained compared to Spatial 2D


#### On the idempotence of 2D & 3D Space Views
In the early prototype 2D and 3D Space Views were separate since they would use different
renderers - 3D Space Views were driven by `three-d`, 2D Space Views by egui directly.
With the advent or `re_renderer`, this distinction was no longer necessary and indeed a hindrance.
Like most modern renderer, `re_renderer` does not distinguish 2D and 3D rendering at a fundamental level
(albeit we might add some limited awareness in the future) since shader, hardware acceleration and
data structures are all fundamentally the same.

If the root of a 2D Space View has a camera projection we can have a defined way of displaying any 3D content.
Therefore, all 3D content can be displayed in a 2D Space View.

Vice versa, if an entity in a 3D space defines camera intrinsics, any 2D contents under it can be previewed
in 3D space. Again, there is no point in putting a limit on what is displayed there.

Arguments for distinguishing 2D and 3D views anyways:
* Control of the `Eye` is vastly different
* Most users have expectations on what a 2D view looks and feels
  * Heuristics of how things are displayed by default may vary wildly
    * e.g. a depth image in 3D should be displayed as a point cloud whenever possible, whereas in 2D this makes rarely sense
* 2D and 3D may have different properties regarding grid lines, background and rendering effects (?)
  * the later is doubtful since again 3D content may be displayed in 2D views


Note that the strong overlaps between 2D and 3D views constrain re-usability points in the design
of `SpaceViewClass`


### Space View State
In addition to blueprint stored data, a space view has a class specific `SpaceViewState`
which stored ephemeral state that is not persisted as part of the blueprint.
This is typically used for animation/transition state.

⚠️ As of writing, we're using this also for state that *should* be be persisted and needs to be moved to
blueprint components.

### `ViewPartSystem`
A `ViewPartSystem` defines how a given archetype is processed during the scene buildup.
Every frame, an instance for every registered `ViewPartSystem` is instantiated.
During instantiation it can access the query results for its archetype and emit `re_renderer` drawables
as well as custom state that can be processed during the `SpaceViewClass`'s drawing method.

TODO: talk about registration
TODO: talk about shared state more
TODO(andreas): Expand drawables concept

Note on naming:
`ViewPartSystem` was called `ScenePart` in earlier versions since it formed a _part_ of a per-frame built-up _Scene_.
We discarded _Scene_ since in most applications scenes are permanent and not per-frame.
However, we determined that they still make up the essential parts of a `SpaceView`.
Their behavior is a match to what in ECS implementations is referred to as a System -
i.e. an object or function that queries a set of components (an Archetype) and executes some logic as a result.

### `ViewContextSystem`
Similarly to `ViewPartSystem`, all registered `ViewContextSystem` are instantiated every frame.
Instantiation happens before `ViewPartSystem` and can not emit drawables, only set custom data.
The results are available during `ViewPartSystem` execution.
This is used e.g. to prepare the transform tree.

TODO: talk about registration

### Frame Lifecycle
TODO: update this
Each frame, each `SpaceView` instance builds up a scene. The framework defines a fixed lifecycle for all views.
Given a `SpaceViewClass` `MyClass`:
* default instantiate a new `TypedScene<MyClass>`
  * this contains an instance of ``MyClass::SceneParts` and `MyClass::SceneParts::Context`,
    each of which are collection of default initialized scene parts and contexts respectively
* `MyClass::prepare_populate()`
* `TypedScene<MyClass>::populate()`
  * for each `SceneContextPart` call `populate` (can be parallelized in the future!)
  * for each `ScenePart` call `populate`, passing in all contexts (can be parallelized in the future!)
* `SpaceViewClass::ui()`, passing in the now populated `TypedScene<MyClass>` as well as a stored instance of `MyClass::State`


### Space View Class Registry
Despite being few in numbers, Space Views Classes are registered on startup.
This is desirable since:
* forces decoupling from other aspects of the Viewer (Viewer should be composable)
* allows for user defined space views


#### User defined Space View Classes
Rust developers can use the Class Registry to register their own Space View types.
We do *not* expect this to be a common workflow, but more of a last resort / highest level
extensibility hooks.

These user defined Space Views have no limitations over built-in Space Views and are able
to completely reimplement existing Space Views if desired.

TODO: update and details
A more common extension point in the future will be extension of the Spatial Space View Classes
by adding new `ScenePart` to them.
