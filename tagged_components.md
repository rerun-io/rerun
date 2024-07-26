We've had a long discussion about leaf (nee "out-of-tree") transforms a couple days back, during which the desire for tagged components came up _a lot_. Again. We keep running into data modeling deadlocks.

In parallel, @katya has been experimenting with the Dataframe View and has hit more of the same issues.

We keep running into data modeling deadlocks, again and again.

---

My guess is that tagged components will open many new avenues when it comes to the designs of our data model and query language, and to an extent even the UX of the viewer itself. As such I think it's really important that we get there sooner rather than later.
It doesn't have to be perfect either: the sooner we get to play with these new tools and refine our data model with them in mind, the sooner we can start stabilizing and ultimately _(gasp)_ standardizing things.

It would also be quite nice to be completely done with major ABI breaking changes ASAP, before we enter the :sparkles: disk-based era :sparkles:.

Thus, here's a quick proposal to move us towards that goal.


## Context

Today, the atomic unit of data in Rerun is a Chunk column.

A Chunk column is fully qualified by two things: a Rerun `ComponentName` and an Arrow `Datatype` (NOTE: I'm intentionally omitting the 3rd qualifier, the "Rerun datatype name", which is a vague internal concept that is not actually materialized most of the time. It doesn't matter for this discussion):
- The component name (`Position3D`, `Color`, `Radius`, …) specifies the semantic of the data, "how things behave".
- The datatype (`[3]Float32`, `Uint32`, …) specifies the memory layout of the data.

The two are for the most part completely orthogonal to one another.

This information is all stored within the column metadata, and denormalized into the store indices as needed.

At runtime, a Rerun system will look for a piece of data by searching for the semantic its interested in, and then interpreting the returned data based on its datatype:
```rust
let data = store.latest_at("my_entity", TimeInt::MAX, "rerun.components.Position3D")?; // untyped (!)
let data = data.try_downcast::<&[[f32; 3]]>()?; // typed
```

All of this works pretty nicely, except for one major limitation: you cannot re-use the same semantics twice (or more) on a single entity.

Example: imagine an entity `"detections"` that holds 3D points and 3D boxes at the same time: they will have to share a batch of `"rerun.components.Position3D"` (for `Points3D::positions` and `Boxes3D::centers`).
Sometimes this is exactly what you want, and things are great. Just enable both the `Points3D` and `Boxes3D` visualizers and you're good to go.
Sometimes it is not what you want, and you're doomed.

This problem infects everything and leads to all kinds of nasty data modeling deadlocks all over the place.


## Proposal

The core idea of this proposal is trivial: to replace the very limited Rerun `ComponentName` with a much more flexible `ComponentDescriptor`.
Of course, this has a ton of ripple effects. I'll try to cover the most important ones, but there's too many to cover them all.

We should be able to get there in small increments that can be merged as they come, with complete feature parity and no visible impact on end-users whatsoever.

Once we're there, we'll be able to start experimenting with all kinds of crazy ideas.


### Data model changes

A Chunk column would still be fully qualified by two bits of information: a Rerun `ComponentDescriptor` (semantics) and an Arrow `Datatype` (memory layout).
All that metadata is stored in the same place as before (column metadata and/or arrow schema).

A `ComponentDescriptor` would look like the following:
```rust
/// A [`ComponentDescriptor`] fully describes the semantics of a column of data.
pub struct ComponentDescriptor {
    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    archetype_name: Option<ArchetypeName>,

    /// Semantic name associated with this data.
    ///
    /// Example: `rerun.components.Position3D`.
    component_name: ComponentName,

    /// Optional label to further qualify the data.
    ///
    /// Example: "postions".
    //
    // TODO: Maybe it's a dedicated type or an `InternedString` or w/e, doesn't matter.
    tag: Option<String>,
}

// NOTE: Take a careful look at this implementation, so you know what I mean later in this doc.
//
// Examples:
// * `rerun.archetypes.Points3D::rerun.components.Position3D#positions`
// * `rerun.components.Translation3D#translation`
// * `third_party.barometric_pressure`
impl std::fmt::Display for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ComponentDescriptor {
            archetype_name,
            component_name,
            tag,
        } = self;
        match (archetype_name, component_name, tag) {
            (None, component_name, None) => f.write_str(component_name),
            (Some(archetype_name), component_name, None) => {
                f.write_fmt(format_args!("{archetype_name}::{component_name}"))
            }
            (None, component_name, Some(tag)) => {
                f.write_fmt(format_args!("{component_name}#{tag}"))
            }
            (Some(archetype_name), component_name, Some(tag)) => {
                f.write_fmt(format_args!("{archetype_name}::{component_name}#{tag}"))
            }
        }
    }
}
```

This information could be trivially code generated already today, and is a struct superset of the status quo.

This would already be enough to get rid of our old indicator components.


### IDL changes

I'm going to use `Points3D` and `Mesh3D` for demonstration purposes. They showcase our data modeling issues well, always semantically-walking on each others' toes.

The major change at the IDL level is that the entire `rerun{.blueprint}.components` just goes away.

(NOTE: I've omitted the usual attributes in all the IDL samples below. They haven't changed in any way.)

I.e. `Points3D` turns from this:
```c
table Points3D {
  positions: [rerun.components.Position3D] (/* … */);

  radii: [rerun.components.Radius] (/* … */);
  colors: [rerun.components.Color] (/* … */);

  labels: [rerun.components.Text] (/* … */);
  class_ids: [rerun.components.ClassId] (/* … */);
  keypoint_ids: [rerun.components.KeypointId] (/* … */);
}
```
into this (the generated `ComponentDescriptor`s are shown as comments):
```c
table Points3D {
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Position3D#positions"
  positions: [rerun.datatypes.Vec3D] ("attr.rerun.component": "rerun.components.Position3D", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Radius#radii"
  radii: [rerun.datatypes.Float32] ("attr.rerun.component": "rerun.components.Radius", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Color#colors"
  colors: [rerun.datatypes.UInt32] ("attr.rerun.component": "rerun.components.Color", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Label#labels"
  labels: [rerun.datatypes.Utf8] ("attr.rerun.component": "rerun.components.Label", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.ClassId#class_ids"
  class_ids: [rerun.datatypes.UInt16] ("attr.rerun.component": "rerun.components.ClassId", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.KeypointId#keypoint_ids"
  keypoint_ids: [rerun.datatypes.UInt16] ("attr.rerun.component": "rerun.components.KeypointId", /* … */);
}
```

`Mesh3D` turns from this:
```c
table Mesh3D {
  vertex_positions: [rerun.components.Position3D] (/* … */);

  triangle_indices: [rerun.components.TriangleIndices] (/* … */);
  vertex_normals: [rerun.components.Vector3D] (/* … */);

  vertex_colors: [rerun.components.Color] (/* … */);
  vertex_texcoords: [rerun.components.Texcoord2D] (/* … */);
  albedo_factor: [rerun.components.AlbedoFactor] (/* … */);
  class_ids: [rerun.components.ClassId] (/* … */);
}
```
into this (the generated `ComponentDescriptor`s are shown as comments):
```c
table Mesh3D {
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Position3D#vertex_positions"
  vertex_positions: [rerun.datatypes.Vec3D] ("attr.rerun.component": "rerun.components.Position3D", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.TriangleIndices#triangle_indices"
  triangle_indices: [rerun.datatypes.UVec3D] ("attr.rerun.component": "rerun.components.TriangleIndices", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Vector3D#vertex_normals"
  vertex_normals: [rerun.datatypes.Vec3D] ("attr.rerun.component": "rerun.components.Vector3D", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#vertex_colors"
  vertex_colors: [rerun.datatypes.UInt32] ("attr.rerun.component": "rerun.components.Color", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.TexCoords2D#vertex_texcoords"
  vertex_texcoords: [rerun.datatypes.Vec2D] ("attr.rerun.component": "rerun.components.TexCoords2D", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#albedo_factor"
  albedo_factor: [rerun.datatypes.UInt32] ("attr.rerun.component": "rerun.components.AlbedoFactor", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.ClassId#class_ids"
  class_ids: [rerun.datatypes.UInt16] ("attr.rerun.component": "rerun.components.ClassId", /* … */);

}
```

### Logging changes

<!-- TODO -->
<!-- * Logging -->
<!--   * Logging archetypes -->
<!--   * Logging components -->

Logging archetypes will yield fully-specified `ComponentDescriptor`s:
```python
rr.log(
    "points_and_mesh",
    rr.Points3D(
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Position3D#positions"
        [[0, 0, 0], [1, 1, 1]],
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Radius#radii"
        radii=10,
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Color#colors"
        colors=[1, 1, 1],
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Label#labels"
        labels="some_label",
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.ClassId#class_ids"
        class_ids=42,
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.KeypointId#keypoint_ids"
        keypoint_ids=666,
    ),
)

rr.log(
    "points_and_mesh",
    rr.Mesh3D(
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Position3D#vertex_positions"
        vertex_positions=[[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Vector3D#vertex_normals"
        vertex_normals=[0.0, 0.0, 1.0],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#vertex_colors"
        vertex_colors=[[0, 0, 255], [0, 255, 0], [255, 0, 0]],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.TriangleIndices#triangle_indices"
        triangle_indices=[2, 1, 0],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#albedo_factor"
        albedo_factor=[32, 32, 32],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.ClassId#class_ids"
        class_ids=420,
    ),
)
```

Logging components directly omits the archetype part of the descriptor:
```python
rr.log(
    "points_and_mesh",
    rr.components.Translation3D(
        # ComponentDescriptor: "rerun.components.Translation3D#translation"
        translation=[1, 2, 3],
    ),
)
```


A third-party ad-hoc component might not even have a tag at all..:
```python
rr.log(
    "points_and_mesh",
    # ComponentDescriptor: "third_party.size"
    rr.AnyValues({"third_party.size": 42}),
)
```

..although we could expose ways of setting one:
```python
rr.log(
    "points_and_mesh",
    # ComponentDescriptor: "third_party.size#some_tag"
    rr.AnyValues({"third_party.size": 42}, "tag": "some_tag"),
)
```

### Store changes

Columns are now uniquely identified by a `(ComponentDescriptor, ArrowDatatype)` pair (as opposed to `(ComponentName, ArrowDatatype)` today).

This means we never overwrite data from an archetype with data from another one. We store everything, we can do whatever we want.

The batcher and other compaction systems will never merge two columns with different descriptors.

Indexing-wise, the store will add an extra layer of indices for tags (`ComponentDescriptor::tag`).
That is trivial to implement and pretty cheap both compute and space wise.


### Query changes

Queries don't look for a `ComponentName` anymore, they look for fully or partially filled `ComponentDescriptor`s instead.

E.g. to look for all columns with position semantics:
- You used to do this:
```rust
latest_at("my_entity", TimeInt::MAX, "rerun.components.Position3D")
```
- You would now do this instead:
```rust
// LatestAt(TimeInt::MAX) @ "my_entity" for (*, "rerun.components.Position3D", *)
latest_at("my_entity", TimeInt::MAX, ComponentDescriptorPattern {
    archetype_name: None, // == any
    component_name: Some("rerun.components.Position3D"), // == any
    tag: None, // == any
})
```

Here's a few example queries using the `Points3D` and `Mesh3D` data we've logged earlier:
```rust
LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, *, *):
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Position3D", "positions" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Radius", "radii" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Color", "colors" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Label", "labels" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.ClassId", "class_ids" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.KeypointId", "keypoint_ids" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Position3D", "vertex_positions" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Vector3D", "vertex_normals" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "vertex_colors" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.TriangleIndices", "triangle_indices" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "albedo_factor" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.ClassId", "class_ids" }


LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, "rerun.components.Position3D", *):
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Position3D", "positions" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Position3D", "vertex_positions"  }


LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, "rerun.components.Color", *):
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Color", "colors" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "vertex_colors" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "albedo_factor" }


LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, "rerun.components.Color", "albedo_factor"):
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "albedo_factor" }
```

It's basically pattern matching.

This should be fairly trivial to implement on the query side.


### Viewer changes

<!-- TODO: For each visualizer, the visualizer not only lists all the pieces of data it depends on and their `ComponentDescriptor`s, but it also let the user change the different parts of the tag in a way that makes sense / is compatible! -->



Today, each visualizer indicates the `ComponentName` it used to fetch a given piece of data:
<!-- TODO: pic -->

In that world, each visualizer would not only show the `ComponentDescriptor` used to source data, but also allow the user to override the descriptors's `archetype_name` and `tag` fields. Want to use your `vertex_colors` as `edge_colors`? No problem!

## Examples


### `SolidColor`, `EdgeColor`, `VertexColor`, etc

Just use tags!

```c
table SomeShapes3D {
  // ComponentDescriptor: "rerun.archetypes.SomeShapes3D::rerun.components.Color#solid_colors"
  solid_colors: [rerun.datatypes.Rgba32] ("attr.rerun.component": "rerun.components.Color", /* … */);
  // ComponentDescriptor: "rerun.archetypes.SomeShapes3D::rerun.components.Color#edge_colors"
  edge_colors: [rerun.datatypes.Rgba32] ("attr.rerun.component": "rerun.components.Color", /* … */);
  // ComponentDescriptor: "rerun.archetypes.SomeShapes3D::rerun.components.Color#vertex_colors"
  vertex_colors: [rerun.datatypes.Rgba32] ("attr.rerun.component": "rerun.components.Color", /* … */);
}
```


### `Transform3D` vs. `LeafTransform3D`

Share the same exact code between the two, all the way. Just change the `archetype_name` part of the `ComponentDescriptor` at log time.

In-tree:
```python
rr.log(
    "moon",
    # ComponentDescriptor: "rerun.archetypes.Transform3D::rerun.components.Rotation#rotation"
    rr.Transform3D(rotation=rr.Quaternion(xyzw=[0, -0.3826834, 0, 0.9238796])),
)
```

Out-of-tree:
```python
rr.log(
    "moon",
    # `rr.LeafTransform3D` is just a helper that calls `rr.Transform3D` but sets the `archetype_name`
    # to `LeafTransform3D` instead.
    #
    # The `TransformContext` will make use of that information at runtime in order to dispatch things
    # appropriately.
    #
    # ComponentDescriptor: "rerun.archetypes.LeafTransform3D::rerun.components.Rotation#rotation"
    rr.LeafTransform3D(rotation=rr.Quaternion(xyzw=[0, -0.3826834, 0, 0.9238796])),
)
```

## FAQ

### What becomes of indicator components?

They're gone; they're just redundant at that point: use the `archetype_name` field in the `ComponentDescriptor` instead.


### What about datatype conversions?

As far as I can tell, datatype conversions is a completely orthogonal problem.

"Tagged components" is about sharing semantics across columns, "datatype conversions" is about making it easy to change the memory layout of a column.


### What about blueprint defaults/overrides?

Mostly nothing changes, except now the blueprint has an opportunity to define a default value for all tags or a specific one, or both.

```rust
blueprint.set_default("*::rerun.components.Color#*", Color::Blue);
blueprint.set_default("*::rerun.components.Color#vertex_colors", Color::Green);
```


### What about the DataframeView?

The dataframe view now has all the information it needs to properly distinguish between data with similar semantics.





































We've had a long discussion about leaf (nee "out-of-tree") transforms a couple days back, during which the desire for tagged components came up _a lot_. Again. We keep running into data modeling deadlocks.

In parallel, @gavrelina  has been experimenting with the Dataframe View and has hit more of the same issues.

We keep running into data modeling deadlocks, again and again.

---

My guess is that tagged components will greatly influence the design of our data model and query language, and to an extent even the UX of the viewer itself. As such I think it's really important that we get there sooner rather than later.
It doesn't have to be perfect either: the sooner we get to play with these new tools and refine our data model with them in mind, the sooner we can start stabilizing and ultimately _(gasp)_ standardizing things.

It would also be quite nice to be completely done with major ABI breaking changes ASAP, before we enter the :sparkles: disk-based era :sparkles:.

Thus, here's a quick proposal to move us towards that goal.

---

## Context

Today, the atomic unit of data in Rerun is a Chunk column.

A Chunk column is fully qualified by two things: a Rerun `ComponentName` and an Arrow `Datatype` (NOTE: I'm intentionally omitting the 3rd qualifier, the "Rerun datatype name", which is a vague internal concept that is not actually materialized most of the time. It doesn't matter for this discussion.):
- The component name (`Position3D`, `Color`, `Radius`, …) specifies the semantic of the data, "how things behave".
- The datatype (`[3]Float32`, `Uint32`, …) specifies the memory layout of the data.

The two are for the most part completely orthogonal to one another.

This information is all stored within the column metadata, and denormalized into the store indices as needed.

At runtime, a Rerun system will look for a piece of data by searching for the semantic its interested in, and then interpreting the returned data based on its datatype:
```rust
let data = store.latest_at("my_entity", TimeInt::MAX, "rerun.components.Position3D")?; // untyped (!)
let data = data.try_downcast::<&[[f32; 3]]>()?; // typed
```

All of this works pretty nicely, except for one major limitation: you cannot re-use the same semantics twice (or more) on a single entity.

Example: imagine an entity `"detections"` that holds both 3D points and 3D boxes at the same time: they will have to share a batch of `"rerun.components.Position3D"` (for `Points3D::positions` and `Boxes3D::centers`).
Sometimes this is exactly what you want, and things are great. Just enable both the `Points3D` and `Boxes3D` visualizers and you're good to go.
Sometimes it is not what you want, and you're doomed.

This problem infects everything and leads to all kinds of nasty data modeling deadlocks all over the place.


## Proposal

The core idea of this proposal is trivial: to replace the very limited Rerun `ComponentName` with a much more flexible `ComponentDescriptor`.
Of course, this has a ton of ripple effects. I'll try to cover the most important ones, but there's too many to cover them all.

We should be able to get there in small increments that can be merged as they come, with complete feature parity and no visible impact on end-users whatsoever.

Once we're there, we'll be able to start experimenting with all kinds of crazy ideas.


### Data model changes

A Chunk column would still be fully qualified by two bits of information: a Rerun `ComponentDescriptor` (semantics) and an Arrow `Datatype` (memory layout).
All that metadata is stored in the same place as before (column metadata and/or arrow schema).

A `ComponentDescriptor` would look like the following:
```rust
/// A [`ComponentDescriptor`] fully describes the semantics of a column of data.
pub struct ComponentDescriptor {
    /// Optional name of the `Archetype` associated with this data.
    ///
    /// `None` if the data wasn't logged through an archetype.
    ///
    /// Example: `rerun.archetypes.Points3D`.
    archetype_name: Option<ArchetypeName>,

    /// Semantic name associated with this data.
    ///
    /// Example: `rerun.components.Position3D`.
    component_name: ComponentName,

    /// Optional label to further qualify the data.
    ///
    /// Example: "postions".
    //
    // TODO: Maybe it's a dedicated type or an `InternedString` or w/e, doesn't matter.
    tag: Option<String>,
}

// NOTE: Take a careful look at this implementation, so you know what I mean later in this doc.
//
// Examples:
// * `rerun.archetypes.Points3D::rerun.components.Position3D#positions`
// * `rerun.components.Translation3D#translation`
// * `third_party.barometric_pressure`
impl std::fmt::Display for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ComponentDescriptor {
            archetype_name,
            component_name,
            tag,
        } = self;
        match (archetype_name, component_name, tag) {
            (None, component_name, None) => f.write_str(component_name),
            (Some(archetype_name), component_name, None) => {
                f.write_fmt(format_args!("{archetype_name}::{component_name}"))
            }
            (None, component_name, Some(tag)) => {
                f.write_fmt(format_args!("{component_name}#{tag}"))
            }
            (Some(archetype_name), component_name, Some(tag)) => {
                f.write_fmt(format_args!("{archetype_name}::{component_name}#{tag}"))
            }
        }
    }
}
```

This information could be trivially code generated already today, and is a strict superset of the status quo.

This would already be enough to get rid of our old indicator components.


### IDL changes

I'm going to use `Points3D` and `Mesh3D` for demonstration purposes. They showcase our data modeling issues well, always semantically-walking on each others' toes.

The major change at the IDL level is that the entire `rerun{.blueprint}.components` layer just goes away.

(NOTE: I've omitted the usual attributes in all the IDL samples below. They haven't changed in any way.)

I.e. `Points3D` turns from this:
```c
table Points3D {
  positions: [rerun.components.Position3D] (/* … */);

  radii: [rerun.components.Radius] (/* … */);
  colors: [rerun.components.Color] (/* … */);

  labels: [rerun.components.Text] (/* … */);
  class_ids: [rerun.components.ClassId] (/* … */);
  keypoint_ids: [rerun.components.KeypointId] (/* … */);
}
```
into this (the generated `ComponentDescriptor`s are shown as comments):
```c
table Points3D {
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Position3D#positions"
  positions: [rerun.datatypes.Vec3D] ("attr.rerun.component": "rerun.components.Position3D", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Radius#radii"
  radii: [rerun.datatypes.Float32] ("attr.rerun.component": "rerun.components.Radius", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Color#colors"
  colors: [rerun.datatypes.UInt32] ("attr.rerun.component": "rerun.components.Color", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Label#labels"
  labels: [rerun.datatypes.Utf8] ("attr.rerun.component": "rerun.components.Label", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.ClassId#class_ids"
  class_ids: [rerun.datatypes.UInt16] ("attr.rerun.component": "rerun.components.ClassId", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.KeypointId#keypoint_ids"
  keypoint_ids: [rerun.datatypes.UInt16] ("attr.rerun.component": "rerun.components.KeypointId", /* … */);
}
```

`Mesh3D` turns from this:
```c
table Mesh3D {
  vertex_positions: [rerun.components.Position3D] (/* … */);

  triangle_indices: [rerun.components.TriangleIndices] (/* … */);
  vertex_normals: [rerun.components.Vector3D] (/* … */);

  vertex_colors: [rerun.components.Color] (/* … */);
  vertex_texcoords: [rerun.components.Texcoord2D] (/* … */);
  albedo_factor: [rerun.components.AlbedoFactor] (/* … */);
  class_ids: [rerun.components.ClassId] (/* … */);
}
```
into this (the generated `ComponentDescriptor`s are shown as comments):
```c
table Mesh3D {
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Position3D#vertex_positions"
  vertex_positions: [rerun.datatypes.Vec3D] ("attr.rerun.component": "rerun.components.Position3D", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.TriangleIndices#triangle_indices"
  triangle_indices: [rerun.datatypes.UVec3D] ("attr.rerun.component": "rerun.components.TriangleIndices", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Vector3D#vertex_normals"
  vertex_normals: [rerun.datatypes.Vec3D] ("attr.rerun.component": "rerun.components.Vector3D", /* … */);

  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#vertex_colors"
  vertex_colors: [rerun.datatypes.UInt32] ("attr.rerun.component": "rerun.components.Color", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.TexCoords2D#vertex_texcoords"
  vertex_texcoords: [rerun.datatypes.Vec2D] ("attr.rerun.component": "rerun.components.TexCoords2D", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#albedo_factor"
  albedo_factor: [rerun.datatypes.UInt32] ("attr.rerun.component": "rerun.components.AlbedoFactor", /* … */);
  // ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.ClassId#class_ids"
  class_ids: [rerun.datatypes.UInt16] ("attr.rerun.component": "rerun.components.ClassId", /* … */);

}
```

### Logging changes


Logging archetypes will yield fully-specified `ComponentDescriptor`s:
```python
rr.log(
    "points_and_mesh",
    rr.Points3D(
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Position3D#positions"
        [[0, 0, 0], [1, 1, 1]],
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Radius#radii"
        radii=10,
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Color#colors"
        colors=[1, 1, 1],
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.Label#labels"
        labels="some_label",
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.ClassId#class_ids"
        class_ids=42,
        # ComponentDescriptor: "rerun.archetypes.Points3D::rerun.components.KeypointId#keypoint_ids"
        keypoint_ids=666,
    ),
)

rr.log(
    "points_and_mesh",
    rr.Mesh3D(
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Position3D#vertex_positions"
        vertex_positions=[[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Vector3D#vertex_normals"
        vertex_normals=[0.0, 0.0, 1.0],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#vertex_colors"
        vertex_colors=[[0, 0, 255], [0, 255, 0], [255, 0, 0]],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.TriangleIndices#triangle_indices"
        triangle_indices=[2, 1, 0],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.Color#albedo_factor"
        albedo_factor=[32, 32, 32],
        # ComponentDescriptor: "rerun.archetypes.Mesh3D::rerun.components.ClassId#class_ids"
        class_ids=420,
    ),
)
```

Logging components directly omits the archetype part of the descriptor:
```python
rr.log(
    "points_and_mesh",
    rr.components.Translation3D(
        # ComponentDescriptor: "rerun.components.Translation3D#translation"
        translation=[1, 2, 3],
    ),
)
```


A third-party ad-hoc component might not even have a tag at all..:
```python
rr.log(
    "points_and_mesh",
    # ComponentDescriptor: "third_party.size"
    rr.AnyValues({"third_party.size": 42}),
)
```

..although we could expose ways of setting one:
```python
rr.log(
    "points_and_mesh",
    # ComponentDescriptor: "third_party.size#some_tag"
    rr.AnyValues({"third_party.size": 42}, "tag": "some_tag"),
)
```

### Store changes

Columns are now uniquely identified by a `(ComponentDescriptor, ArrowDatatype)` pair (as opposed to `(ComponentName, ArrowDatatype)` today).

This means we never overwrite data from an archetype with data from another one. We store everything, we can do whatever we want.

The batcher and other compaction systems will never merge two columns with different descriptors.

Indexing-wise, the store will add an extra layer of indices for tags (`ComponentDescriptor::tag`).
That is trivial to implement and pretty cheap both compute and space wise.


### Query changes

Queries don't look for a `ComponentName` anymore, they look for fully or partially filled `ComponentDescriptor`s instead.

E.g. to look for all columns with position semantics:
- You used to do this:
  ```rust
  latest_at("my_entity", TimeInt::MAX, "rerun.components.Position3D")
  ```
- You would now do this instead:
  ```rust
  // LatestAt(TimeInt::MAX) @ "my_entity" for (*, "rerun.components.Position3D", *)
  latest_at("my_entity", TimeInt::MAX, ComponentDescriptorPattern {
      archetype_name: None, // == any
      component_name: Some("rerun.components.Position3D"), // == any
      tag: None, // == any
  })
  ```

Here's a few example queries using the `Points3D` and `Mesh3D` data we've logged earlier:
```rust
LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, *, *):
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Position3D", "positions" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Radius", "radii" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Color", "colors" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Label", "labels" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.ClassId", "class_ids" }
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.KeypointId", "keypoint_ids" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Position3D", "vertex_positions" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Vector3D", "vertex_normals" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "vertex_colors" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.TriangleIndices", "triangle_indices" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "albedo_factor" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.ClassId", "class_ids" }


LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, "rerun.components.Position3D", *):
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Position3D", "positions" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Position3D", "vertex_positions"  }


LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, "rerun.components.Color", *):
- ComponentDescriptor { "rerun.archetypes.Points3D", "rerun.components.Color", "colors" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "vertex_colors" }
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "albedo_factor" }


LatestAt(TimeInt::MAX) @ "points_and_mesh" for (*, "rerun.components.Color", "albedo_factor"):
- ComponentDescriptor { "rerun.archetypes.Mesh3D", "rerun.components.Color", "albedo_factor" }
```

It's basically pattern matching.

This should be fairly trivial to implement on the query side.


### Viewer changes

Today, each visualizer indicates the `ComponentName` it uses to fetch a given piece of data:
<!-- TODO: pic -->

In that world, each visualizer would not only show the `ComponentDescriptor` used to source data, but also allow the user to override the descriptors's `archetype_name` and `tag` fields. Want to use your `vertex_colors` as `edge_colors`? No problem!

## Examples


### `SolidColor`, `EdgeColor`, `VertexColor`, etc

Just use tags!

```c
table SomeShapes3D {
  // ComponentDescriptor: "rerun.archetypes.SomeShapes3D::rerun.components.Color#solid_colors"
  solid_colors: [rerun.datatypes.Rgba32] ("attr.rerun.component": "rerun.components.Color", /* … */);
  // ComponentDescriptor: "rerun.archetypes.SomeShapes3D::rerun.components.Color#edge_colors"
  edge_colors: [rerun.datatypes.Rgba32] ("attr.rerun.component": "rerun.components.Color", /* … */);
  // ComponentDescriptor: "rerun.archetypes.SomeShapes3D::rerun.components.Color#vertex_colors"
  vertex_colors: [rerun.datatypes.Rgba32] ("attr.rerun.component": "rerun.components.Color", /* … */);
}
```


### `Transform3D` vs. `LeafTransform3D`

Share the same exact code between the two, all the way. Just change the `archetype_name` part of the `ComponentDescriptor` at log time.

In-tree:
```python
rr.log(
    "moon",
    # ComponentDescriptor: "rerun.archetypes.Transform3D::rerun.components.Rotation#rotation"
    rr.Transform3D(rotation=rr.Quaternion(xyzw=[0, -0.3826834, 0, 0.9238796])),
)
```

Out-of-tree:
```python
rr.log(
    "moon",
    # `rr.LeafTransform3D` is just a helper that calls `rr.Transform3D` but sets the `archetype_name`
    # to `LeafTransform3D` instead.
    #
    # The `TransformContext` will make use of that information at runtime in order to dispatch things
    # appropriately.
    #
    # ComponentDescriptor: "rerun.archetypes.LeafTransform3D::rerun.components.Rotation#rotation"
    rr.LeafTransform3D(rotation=rr.Quaternion(xyzw=[0, -0.3826834, 0, 0.9238796])),
)
```

## FAQ

### What becomes of indicator components?

They're gone; they're just redundant at that point: use the `archetype_name` field in the `ComponentDescriptor` instead.


### What about datatype conversions?

As far as I can tell, datatype conversions is a completely orthogonal problem.

"Tagged components" is about sharing semantics across columns, "datatype conversions" is about making it easy to change the memory layout of a column.


### What about blueprint defaults/overrides?

Mostly nothing changes, except now the blueprint has an opportunity to define a default value for all tags or a specific one, or both.

```rust
blueprint.set_default("*::rerun.components.Color#*", Color::Blue);
blueprint.set_default("*::rerun.components.Color#vertex_colors", Color::Green);
```


### What about the DataframeView?

The dataframe view now has all the information it needs to properly distinguish between data with similar semantics.
