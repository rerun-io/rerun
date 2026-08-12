---
name: rerun-urdf
description: Drive the Rerun URDF API (rerun.urdf.UrdfTree) to ingest a URDF as a Transform3D layer on a robot recording. Read when logging a robot model, running forward kinematics from joint states, composing a fixed chain for sensor extrinsics, or when the transform tree will not connect from the data alone. Builds on rerun-chunk-processing (stream/lens mechanics) and rerun-data-model (entity paths, timeline, base-vs-layer).
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun URDF ingestion

A URDF gives you a robot's geometry and its kinematic tree.
Ingesting it means two API calls on one `rerun.urdf.UrdfTree`: stream the static model, then drive it with forward kinematics from joint states. The transforms it produces are derived, so they are a **layer**, never base (the `URDF + joints (computed)` row of the `rerun-data-model` table).

This skill is the `UrdfTree` API and the two judgment calls it cannot make for
you: how your joint values map to URDF joints, and how the disconnected frames
in the scene connect to one root. The stream/lens plumbing (`LazyChunkStream`,
`DeriveLens`, `Selector`, writing and optimizing RRDs) is in
`rerun-chunk-processing`; reach for it, do not re-derive it. Nothing below is
tied to a data format: where your joint names, joint values, and calibration
come from is yours to wire in.

## The API

```python
from rerun.urdf import UrdfTree

urdf = UrdfTree.from_file_path(
    urdf_path,
    entity_path_prefix="robot",  # links log under /robot/<link>
    frame_prefix="",  # prepended to every frame name
    static_transform_entity_path="robot/tf_static",
)
```

- `entity_path_prefix` namespaces the entity tree. One per robot instance.
- `frame_prefix` namespaces the **frame names** (`base_link` -> `arm_base_link`).
  Two robots in one recording need different prefixes or their roots collide and
  transforms cross-wire. Leave it empty for a single robot.
- `static_transform_entity_path` is where the URDF's fixed-joint transforms log
  (defaults to `/tf_static`).

The tree is also introspectable (full surface in `help(UrdfTree)`)
For one-off, non-stream use there are `joint.compute_transform(value)`, `joint.compute_transform_columns(values)` (feeds `rr.send_columns`), and `urdf.log_urdf_to_recording()` to log the whole model through the classic logging API (the `animated_urdf` example, `https://github.com/rerun-io/rerun/tree/main/examples/python/animated_urdf`, is that style).

For pipelines, a `UrdfTree` does two things.

**1. Stream the static model.** Emits the visual meshes (`Asset3D`) and the
fixed-joint transforms as chunks. This is the whole "log the URDF" step:

```python
model = (
    urdf.stream(include_joint_transforms=True).drop(  # rest-pose joint transforms too
        content="/robot/**/collision_geometries/**"
    )  # unless you need collision meshes
)
```

Recolor a robot's meshes with a `MutateLens` on `Asset3D:albedo_factor` (see `rerun-chunk-processing`).

**2. Solve forward kinematics.** Given joint names and the matching joint values, it returns one `rerun.urdf.JointTransformBatch` per input row. Each batch is a list of per-joint entries with `parent_frame`, `child_frame`, `translation`, and
`quaternion`:

```python
batches = urdf.compute_joint_transform_batches(names, values, clamp=False)
# names, values: pyarrow arrays, one list per timestamp (names aligned to values)
# clamp=True clamps out-of-limit values and warns, useful while debugging units
```

You will almost always run this inside a stream so it stays columnar and lazy,
via the two-lens pattern: derive the batch, then `scatter=True` it into
`Transform3D.descriptor_translation/quaternion/parent_frame/child_frame`. The
full shape is the "Minimal shape" section below; the `robot_data_preprocessing`
example (References) is a complete working instance. The only URDF-specific
part is the `compute_joint_transform_batches` call inside the first lens;
everything else is generic stream mechanics (`rerun-chunk-processing`).

Merge the model stream and the FK stream, `collect(optimize=OBJECT_STORE)`, and
`write_rrd(..., recording_id=<segment_id>)`. The `recording_id` must equal the
base segment id or the layer never attaches; `application_id` is discarded on
registration.

## Mapping joint state to URDF joints (you supply this; the data will not)

`compute_joint_transform_batches` is only as right as the `names`/`values` you
hand it, and the mapping is not in the URDF. Three things go wrong silently:

- **Order.** Build an explicit `names` array aligned to the `values` you read.
  Never assume your message's field order matches the URDF's `<joint>` order.
- **Count.** The URDF's non-`fixed` joint count rarely equals your reported value
  count. A gripper sent as one value is often two prismatic joints in the URDF;
  a mimic joint may be omitted from the message. Reconcile explicitly, and use
  the API to do it: iterate `urdf.joints()`, partition by `joint_type` and
  `mimic`. A joint with `mimic` set derives its value from the driver joint as
  `driver * multiplier + offset`; feed it that, not a message field. Confirm
  the count against `urdf.joints()`, not the message length.
- **Units.** URDF joints are radians (revolute) and meters (prismatic). Convert
  if your source differs.

Get any of these wrong and FK runs and writes a confident, wrong pose.

## Make the joint states readable first

Where the joint values come from is not this skill's problem, but a dead joint
source produces an empty FK layer **with no error**: a source path that
matched nothing, a decoder that yielded zero rows, or a reader that dropped the
message silently. Whatever the source, confirm the joint-state stream yields
rows before debugging FK. The importer skill for your source format covers its
own empty-stream failure modes.

## How transforms compose (reason about this before logging anything)

Rerun resolves a pose by chaining transforms from a frame up to a root. There
are two ways an edge in that chain gets defined, and a URDF ingest mixes both:

- **By entity-path hierarchy.** A `Transform3D` on `/a/b` with no frame names is
  the transform of `/a/b` relative to its parent path `/a`. Composition follows
  the entity tree.
- **By explicit frame graph.** A `Transform3D` that carries `parent_frame` and
  `child_frame` defines an edge between two **named frames**, independent of
  where in the entity tree it is logged. URDF FK uses this: every joint
  transform names its parent and child link frames.

So a URDF ingest is a graph of named frames. An edge exists only if some
`Transform3D` names that exact `parent_frame -> child_frame` pair. A frame with
no incoming edge is a root. The viewer renders every root at the world origin,
which is why two unconnected robots silently overlap instead of erroring.

## Resolving the transform forest (the part the data cannot always give you)

A URDF is **one tree rooted at its base link**. FK and fixed joints supply every
edge inside that tree. A real scene is a **forest** of roots the URDF never
connects: a world or scene frame, each robot's base, every camera or sensor
frame. The edges that join those roots come from calibration (extrinsics in a
sidecar, a TF static publisher, a hand-eye result), not from the URDF, and some
are simply absent. A single connected tree is not always solvable. Resolve it
deliberately:

1. **Enumerate every frame.** Iterate `urdf.joints()` and collect the
   `parent_link`/`child_link` pairs (the in-tree edges, frame-prefixed);
   `urdf.root_link()` is that URDF's root. Add every sensor/world frame the
   scene needs (from the `rerun-data-model` table). Decide the one intended
   root.
2. **Classify each edge by source.** In-URDF edges (FK joints, `fixed` joints)
   come from the URDF plus joint values. Inter-root edges (root to each robot
   base, root to each fixed sensor, an arm link to a wrist-mounted camera) come
   from calibration and you must log them yourself.
3. **Compose fixed chains from the URDF** when you need the transform between two
   links joined only by `fixed` joints (a camera bracket, a tool mount): walk
   parent links across `fixed` joints via `urdf.joints()`, turning each joint's
   `origin_xyz`/`origin_rpy` into a homogeneous matrix and multiplying along
   the chain. If the walk cannot reach
   the target link, **stop and say so** ("no fixed chain from A to B; stuck at
   C"). A broken chain is a wrong pose, not a missing one.
4. **Build the edge set and find the roots.** Collect every `parent_frame ->
child_frame` pair you will log (URDF + calibration). Walk parents from each
   frame; any frame that does not reach the intended root is an unconnected root
   and names a **missing edge**. This is a pure graph check you can run before
   writing the RRD.
5. **Resolve every missing edge, or fail loudly.** For each one:
    - If calibration supplies the transform, log it (next section).
    - If the data does not, the tree is unsolvable. Do **not** leave the frame
      disconnected (it collapses onto the origin and reads as one merged scene).
      Either abort and name the missing edge, or log identity and emit a loud
      warning naming the assumed edge. State which you did.
6. **Match frame names across sources.** FK-derived `parent_frame`/`child_frame`
   must equal the names `urdf.stream()` emits for the static geometry, and your
   calibration edges must use those same names, or links float off the mesh. The
   `frame_prefix` is what keeps them identical; reuse it everywhere.

A correct ingest has exactly one root, and a path from every frame to it.

## Logging a connection correctly

A connecting edge is a **static** `Transform3D` carrying the bridging frame
names. Static (no time index) so it holds for the whole recording; the frame
names, not the entity path, are what create the graph edge. This
`Chunk.from_columns` is the rare **sidecar exception** — a calibration transform
no reader or FK lens can produce; do not generalize it to transforms a reader
emits (a `frame_transforms` topic → `Transform3D`) or that FK derives:

```python
import rerun as rr
from rerun.experimental import Chunk, LazyChunkStream

# world -> this robot's base, from your calibration (translation + xyzw quaternion)
edge = Chunk.from_columns(
    "/world/robot_base",  # any sensible bridging entity path
    indexes=[],  # no index == static
    columns=rr.Transform3D.columns(
        translation=[translation],
        quaternion=[quaternion_xyzw],
        parent_frame=["world"],  # must match the root frame name
        child_frame=["arm_base_link"],  # must match the URDF root frame (prefixed)
    ),
)
edges = LazyChunkStream.from_iter([edge])  # merge alongside model + FK streams
```

Log a fixed-chain result (step 3) the same way, with `parent_frame` and
`child_frame` set to the two link frames the chain spans. Merge all edge chunks
into the same recording as the model and FK streams so they share the graph.

## Minimal shape (one robot, generic joint source)

```python
import rerun as rr
from rerun.experimental import DeriveLens, LazyChunkStream, OptimizationProfile, Selector
from rerun.urdf import UrdfTree

urdf = UrdfTree.from_file_path(urdf_path, entity_path_prefix="robot", static_transform_entity_path="robot/tf_static")
model = urdf.stream(include_joint_transforms=True).drop(content="/robot/**/collision_geometries/**")

joints = source_joint_state_stream()  # your reader; one message column of names+values
fk = (
    joints
    .lenses(
        DeriveLens(JOINT_MSG_COMPONENT, output_entity="/tmp/batches").to_component(
            "rerun.urdf.JointTransformBatch",
            Selector(".").pipe(lambda msgs: urdf.compute_joint_transform_batches(read_names(msgs), read_values(msgs))),
        ),
        content=JOINT_SOURCE_PATH,
        output_mode="forward_all",
    )
    .lenses(
        DeriveLens("rerun.urdf.JointTransformBatch", output_entity="/robot/transforms", scatter=True)
        .to_component(rr.Transform3D.descriptor_translation(), Selector("[].translation"))
        .to_component(rr.Transform3D.descriptor_quaternion(), Selector("[].quaternion"))
        .to_component(rr.Transform3D.descriptor_parent_frame(), Selector("[].parent_frame"))
        .to_component(rr.Transform3D.descriptor_child_frame(), Selector("[].child_frame")),
        content="/tmp/batches",
        output_mode="drop_unmatched",
    )
    .filter(content="/robot/transforms")
)

LazyChunkStream.merge(model, fk).collect(optimize=OptimizationProfile.OBJECT_STORE).write_rrd(
    out_path,
    application_id="urdf",
    recording_id=segment_id,
)
```

`read_names`, `read_values`, `JOINT_MSG_COMPONENT`, and `JOINT_SOURCE_PATH` are
the only data-specific pieces, and the mapping section above is what makes them
correct.

## Gotchas that cause real failures

1. Empty layer, no error: dead joint-state source (decoded to zero rows; see the importer skill for your format) or wrong `JOINT_SOURCE_PATH`/component name.
2. Confident wrong pose: joint count, order, or units wrong.
3. Layer writes but never attaches: `recording_id != segment_id`.
4. Frames collide: two robots sharing a `frame_prefix`.
5. Scene looks merged at the origin: unconnected roots logged as identity without a calibration edge.
6. Catalog ingest rejects or misorders chunks: `OBJECT_STORE` optimization skipped.

## References

- `https://github.com/rerun-io/rerun/tree/main/examples/python/robot_data_preprocessing`
  (FK two-lens pattern, two robots + scene URDFs, prefixes, recoloring,
  calibration offsets)
- `https://github.com/rerun-io/rerun/tree/main/examples/python/animated_urdf`
  (classic logging API: `log_urdf_to_recording`, per-joint `compute_transform`)
- `rerun-data-model` (the mapping table this skill consumes)
- the importer skill for your joint-state source format (making the source readable)
- `rerun-chunk-processing` (lens/stream, write/optimize mechanics)
