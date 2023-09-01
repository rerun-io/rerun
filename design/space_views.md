# Space Views
Status: Mostly implemented.


## What are Space Views
Space Views visualize a Data Blueprint, i.e. a set of entities with given properties.
They are represented as freely arrangeable tiles in the Viewport.
Most Space Views are interactive, allowing their data to be explored freely.


## Properties of a Space View
All properties are saved as part of the blueprint.

Changing discards Space View State:
* Space View Class
* root entity path

Freely mutable:
* name
* positioning within layout
* class specific properties
* Data Blueprint

## Root entity path
* root of the transform hierarchy (if any is used)
* may govern heuristics
* available at various stages of ui drawing & system execution build-up (see below)


## Space View State
In addition to blueprint stored data, a space view has a class specific `SpaceViewState`
which stored ephemeral state that is not persisted as part of the blueprint.
This is typically used for animation/transition state.

⚠️ As of writing, we're using this also for state that *should* be be persisted and needs to be moved to
blueprint components.

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
* Text Document
* Time Series

#### Future Space View Class distinction

The fundamental difference between different space views lies in the kinds of axes a view has.
- Data Table (currently text views) have rows and columns with text
- Text Log have rows with logs sorted in time (not 100% sure this is fundamentally different than Data Table)
- Spatial 2D has two orthogonal axes with defined spatial relationships
- Spatial 3D has three orthogonal axes with defined spatial relationships
- Time Series has one time axis and one numeric axis
- Rich Text is a rich text document (linear in top to bottom with wraparound in horizontal)

##### On merging Bar Chart with Spatial 2D
It might take some time to get the Archetype Queries + defaults expressive and and easy to use enough that it makes sense to merge bar chart with spatial 2D. Right now we have the state that the bar chart space view takes a single 1-D tensor and draws a bar chart with x-axis = tensor indices and y-axis = tensor values. It draws boxes with width 1, centered on integers in x, y-min = 0 and y-max = tensor value.

With the right set of primitives a user should be able to manually build a bar chart in a spatial 2D view. For example they might want a stacked bar chart. Talking about bringing in 3D into a bar chart doesn't likely make sense since there probably doesn't exist a camera projection that maps between 3D and the tensor indices axis (x).

One could imagine that we would have heuristics that generate a Data Blueprint for boxes that creates a bar chart from 1-D tensors.

##### On why 2D and 3D space views shouldn't be the same
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

However, they are more different from a users point of view.

First of all 3D data is only viewable in 2D if combined with a suitable projection (could be through perspective projection or by dropping the data of one dimension). The fact that 2D views are rendered in a 3D pipeline using some kind of pseudo depth (draw order) and an implicitly defined orthographic camera, is not top of mind to me as a user. This is something you need considerable exposure to graphics or 3D computer vision to experience as immediately obvious.

Second, the expectations around how to navigate a 2D visualization are quite different from how I expect to navigate a 3D visualization.

### Registering
Registration happens on startup in the viewer owned `SpaceViewClassRegistry`.
The viewer registers all builtin Space View Classes and users may add new types at any point in time.


### Systems

Space View systems are the primary means how a Space View processes entities.
All Space View systems are instantiated and executed every frame.
Each System operates on a statically defined set of archetypes.
Execution is allowed to store arbitrary state for the duration of the frame.

For the moment we have a simple two step framework:

#### `ViewContextSystem`
Instantiation happens before `ViewPartSystem` and can not emit drawables, only set internal state.
The results are available during `ViewPartSystem` execution as well as `SpaceViewClass` drawing.

This is used e.g. to prepare the transform tree.
Each `ViewPartSystem` that knows about this `TransformContext` can then use it to look up transforms.

#### `ViewPartSystem`
Gathers data from the store and emits `re_renderer` draw data for later use in the `SpaceViewClass`'s ui/drawing method.

For convenience, it provides a `data() -> &Any` method to make it easy to expose results other than `re_renderer` draw data
in a generic fashion.

Example:
The `Points2DPart` queries the `Points2D` archetype upon execution and produces as a result `re_renderer::PointCloudDrawData`.
Since points can have ui labels, it also stores `UiLabel` in its own state which the space view class of `ui`
can read out via `Points2DPart::data()` to draw ui labels.

Note on naming:
`ViewPartSystem` was called `ScenePart` in earlier versions since it formed a _part_ of a per-frame built-up _Scene_.
We discarded _Scene_ since in most applications scenes are permanent and not per-frame.
However, we determined that they still make up the essential parts of a `SpaceViewClass`.
Their behavior is a match to what in ECS implementations is referred to as a System -
i.e. an object or function that queries a set of components (an Archetype) and executes some logic as a result.

### Registration
Registration is done via `SpaceViewSystemRegistry` which `SpaceViewClassRegistry` stores for each class.
Space view classes can register their built-in systems upon their own registration via their `on_register` method.
As with space view classes themselves, new systems may be added at runtime.

### Frame Lifecycle
* `SpaceViewClass::prepare_ui`
* default create all registered `ViewContextSystem` into a `ViewContextCollection`
* execute all `ViewContextSystem`
* default create all registered `ViewPartSystem` into a `ViewPartCollection`
* execute all `ViewPartSystem`, giving read access to the `ViewContextSystem`
  * this produces a list or `re_renderer` draw data
* pass all previously assembled objects as read-only into `SpaceViewClass::ui`
  * here the actual rendering via egui happens
    * this typically requires iterating over all `ViewPartSystem` and extract some data either in a generic fashion via `ViewPartSystem::data` or with knowledge of the concrete `ViewPartSystem` types
  * currently, we also pass in all `re_renderer` data since the build up of the `re_renderer` view via `ViewBuilder` is not (yet?) unified

### Space View Class Registry
Despite being few in numbers, Space Views Classes are registered on startup.
This is desirable since:
* forces decoupling from other aspects of the Viewer (Viewer should be composable)
* allows for user defined space views

<!-- https://www.figma.com/file/uFpsPdnEjKbdEv9fQif5mU/Space-View-Structure?type=whiteboard&node-id=603-139&t=B8lmYdoC9j99ZmxJ-4 -->
![Overview diagram of how the basic traits related to each other](https://github.com/rerun-io/rerun/assets/1220815/ffdb1cdf-7efe-47a0-ac38-30262d770e69)


#### User defined Space View Classes
Rust developers can use the Class Registry to register their own Space View types.
We do *not* expect this to be a common workflow, but more of a last resort / highest level
extensibility hooks.

These user defined Space Views have no limitations over built-in Space Views and are able
to completely reimplement existing Space Views if desired.

In the future A more common extension point will be to add custom systems to an existing Space View
emitting re_renderer drawables.
(TODO(andreas): We're lacking API hooks and an example for this!)
