---
title: Logging Data in Rust
order: 5
---

In this section we'll log and visualize our first non-trivial dataset, putting many of Rerun's core concepts and features to use.

In a few lines of code, we'll go from a blank sheet to something you don't see everyday: an animated, interactive, DNA-shaped abacus:
<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

This guide aims to go wide instead of deep.
There are links to other doc pages where you can learn more about specific topics.

At any time, you can checkout the complete code listing for this tutorial [here](https://github.com/rerun-io/rerun/tree/latest/examples/rust/dna/src/main.rs) to better keep track of the overall picture.

## Prerequisites

We assume you have a working Rust environment and have started a new project with the `rerun` dependency. If not, check out the [setup page](rust.md).

For this example in particular, we're going to need all of these:
```toml
[dependencies]
rerun = "0.7.0"
itertools = "0.10"
rand = "0.8"
```

While we're at it, let's get imports out of the way:
```rust
use std::f32::consts::TAU;

use itertools::Itertools as _;

use rerun::{
    components::{Color, LineStrip3D, Point3D, Radius, Transform3D, Vec3D},
    demo_util::{bounce_lerp, color_spiral},
    external::glam,
    time::{Time, TimeType, Timeline},
    transform, MsgSender,
};
```

Already you can see the two most important types we'll interact with:
- [`RecordingStream`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html), our entrypoint into the logging SDK.
- [`MsgSender`](https://docs.rs/rerun/latest/rerun/struct.MsgSender.html), a builder-like type that we'll use to pack our data in order to prep it for logging.


## Starting the viewer
Just run `rerun` to start the [Rerun Viewer](../reference/viewer/overview.md). It will wait for your application to log some data to it. This viewer is in fact a server that's ready to accept data over TCP (it's listening on `0.0.0.0:9876` by default).

Checkout `rerun --help` for more options.

![logging data - waiting for data](https://static.rerun.io/4f83e588d7ca4ba6d09390d6d445f63bb4a73b4e_logging_data2_waiting.png)

## Initializing the SDK

To get going we want to create a [`RecordingStream`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html):
We can do all of this with the [`rerun::RecordingStreamBuilder::new`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.new) function which allows us to name the dataset we're working on by setting its [`ApplicationId`](https://docs.rs/rerun/latest/rerun/struct.ApplicationId.html):

```rust
fn main() {
    let recording =
        rerun::RecordingStreamBuilder::new("rerun_example_dna_abacus").connect(rerun::default_server_addr())?;

    Ok(())
}
```

Among other things, a stable [`ApplicationId`](https://docs.rs/rerun/latest/rerun/struct.ApplicationId.html) will make it so the [Rerun Viewer](../reference/viewer/overview.md) retains its UI state across runs for this specific dataset, which will make our lives much easier as we iterate.

Check out the reference to learn more about how Rerun deals with [applications and recordings](../concepts/apps-and-recordings.md).


## Logging our first points

The core structure of our DNA looking shape can easily be described using two point clouds shaped like spirals.
Add the following to your `main` function:
```rust
const NUM_POINTS: usize = 100;

let (points1, colors1) = color_spiral(NUM_POINTS, 2.0, 0.02, 0.0, 0.1);
let (points2, colors2) = color_spiral(NUM_POINTS, 2.0, 0.02, TAU * 0.5, 0.1);

MsgSender::new("dna/structure/left")
    .with_component(&points1.iter().copied().map(Point3D::from).collect_vec())?
    .with_component(&colors1.iter().copied().map(Color::from).collect_vec())?
    .with_splat(Radius(0.08))?
    .send(&recording)?;

MsgSender::new("dna/structure/right")
    .with_component(&points2.iter().copied().map(Point3D::from).collect_vec())?
    .with_component(&colors2.iter().copied().map(Color::from).collect_vec())?
    .with_splat(Radius(0.08))?
    .send(&recording)?;
```

Run your program with `cargo run` and you should now see this scene in the viewer:

![logging data - first points](https://static.rerun.io/46140c891a60026b3ef9fb0c34fcf63e23199ec7_logging_data3_first_points.png)

_This is a good time to make yourself familiar with the viewer: try interacting with the scene and exploring the different menus._
_Checkout the [Viewer Walkthrough](viewer-walkthrough.md) and [viewer reference](../reference/viewer/overview.md) for a complete tour of the viewer's capabilities._

### Under the hood

Although there's not that much code yet, there's already quite a lot that's happening under the hood.

#### `Entities & hierarchies`

Note the two strings we're passing in when creating our `MsgSender`s: `"dna/structure/left"` & `"dna/structure/right"`.

These are [Entity Paths](../concepts/entity-component.md), which uniquely identify each Entity in our scene. Every Entity is made up of a path and one or more Components.
[Entity paths typically form a hierarchy](../concepts/entity-path.md) which plays an important role in how data is visualized and transformed (as we shall soon see).

#### `Components`

The Rerun [Rust SDK](https://rerun-io.github.io/rerun/docs/rust) works at a lower-level of abstraction than the [Python one](https://ref.rerun.io/docs/python).
In particular, when using the Rust SDK, you work directly with [`components`](https://docs.rs/rerun/latest/rerun/components) instead of higher-level primitives.

By logging multiple components to an Entity, one can build up Primitives that can later be visualized in the viewer.
For more information on how the rerun data model works, refer to our section on [entities and components](../concepts/entity-component.md).

Logging components is a only a matter of calling [`MsgSender::with_component`](https://docs.rs/rerun/latest/rerun/struct.MsgSender.html#method.with_component) using any type that implements the [`Component` trait](https://docs.rs/rerun/latest/rerun/experimental/trait.Component.html). We provide [a few of those](https://docs.rs/rerun/latest/rerun/experimental/trait.Component.html#implementors)).

#### `Batches`

One final observation: notice how we're logging a whole batch of points and colors all at once here.
[Batches of data](../concepts/batches.md) are first-class citizens in Rerun and come with all sorts of performance benefits and dedicated features.
You're looking at one of these dedicated features right now in fact: notice how we're only logging a single radius for all these points, yet somehow it applies to all of them.

---

A _lot_ is happening in these two simple function calls.
Good news is: once you've digested all of the above, logging any other Component will simply be more of the same. In fact, let's log everything else in the scene right now.

## Adding the missing pieces

We can represent the scaffolding using a batch of 3D line segments:
```rust
let all_points = points1.iter().interleave(points2.iter()).copied();
let scaffolding = all_points
    .map(Vec3D::from)
    .chunks(2)
    .into_iter()
    .map(|positions| LineStrip3D(positions.collect_vec()))
    .collect_vec();
MsgSender::new("dna/structure/scaffolding")
    .with_component(&scaffolding)?
    .with_splat(Color::from([128, 128, 128, 255]))?
    .send(&recording)?;
```

Which only leaves the beads:
```rust
use rand::Rng as _;
let mut rng = rand::thread_rng();
let offsets = (0..NUM_POINTS).map(|_| rng.gen::<f32>()).collect_vec();

let (beads, colors): (Vec<_>, Vec<_>) = points1
    .iter()
    .interleave(points2.iter())
    .copied()
    .chunks(2)
    .into_iter()
    .enumerate()
    .map(|(n, mut points)| {
        let (p1, p2) = (points.next().unwrap(), points.next().unwrap());
        let c = bounce_lerp(80.0, 230.0, offsets[n] * 2.0) as u8;
        (
            Point3D::from(bounce_lerp(p1, p2, offsets[n])),
            Color::from_rgb(c, c, c),
        )
    })
    .unzip();
MsgSender::new("dna/structure/scaffolding/beads")
    .with_component(&beads)?
    .with_component(&colors)?
    .with_splat(Radius(0.06))?
    .send(&recording)?;
```

Once again, although we are getting fancier and fancier with our iterator mappings, there is nothing new here: it's all about building out vectors of [`Component`s](https://docs.rs/rerun/latest/rerun/experimental/trait.Component.html) and feeding them to the Rerun API.

![logging data - beads](https://static.rerun.io/60c3c762448f68da3f5fdd7927a6e65e11f5385f_logging_data5_beads.png)

## Animating the beads

### Introducing Time

Up until this point, we've completely set aside one of the core concepts of Rerun: [Time and Timelines](../concepts/timelines.md).

Even so, if you look at your [Timeline View](../reference/viewer/timeline.md) right now, you'll notice that Rerun has kept track of time on your behalf anyways by memorizing when each log call occurred.

![logging data - timeline closeup](https://static.rerun.io/f6dbc83f555597e2bfe946e8228301da82ad4611_logging_data6_timeline.png)

Unfortunately, the logging time isn't particularly helpful to us in this case: we can't have our beads animate depending on the logging time, else they would move at different speeds depending on the performance of the logging process!
For that, we need to introduce our own custom timeline that uses a deterministic clock which we control.

Rerun has rich support for time: whether you want concurrent or disjoint timelines, out-of-order insertions or even data that lives _outside_ of the timeline(s)… you'll find a lot of flexibility in there.

Let's add our custom timeline:
```rust
for i in 0..400 {
    let time = i as f32 * 0.01;

    rec.set_time_seconds("stable_time", time as f64);

    let times = offsets.iter().map(|offset| time + offset).collect_vec();
    let (beads, colors): (Vec<_>, Vec<_>) = points1
        .iter()
        .interleave(points2.iter())
        .copied()
        .chunks(2)
        .into_iter()
        .enumerate()
        .map(|(n, mut points)| {
            let (p1, p2) = (points.next().unwrap(), points.next().unwrap());
            let c = bounce_lerp(80.0, 230.0, times[n] * 2.0) as u8;
            (
                Point3D::from(bounce_lerp(p1, p2, times[n])),
                Color::from_rgb(c, c, c),
            )
        })
        .unzip();

    MsgSender::new("dna/structure/scaffolding/beads")
        .with_component(&beads)?
        .with_component(&colors)?
        .with_splat(Radius(0.06))?
        .send(&recording)?;
}
```

First we use [`RecordingStream::set_time_seconds`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.set_time_seconds) to declare our own custom `Timeline` and set the current timestamp.
You can add as many timelines and timestamps as you want when logging data.

⚠️  If you run this code as is, the result will be.. surprising: the beads are animating as expected, but everything we've logged until that point is gone! ⚠️

![logging data - wat](https://static.rerun.io/a396c8aae1cbd717a3f35472594f789e4829b1ae_logging_data7_wat.png)

Enter...

### Latest At semantics

That's because the Rerun Viewer has switched to displaying your custom timeline by default, but the original data was only logged to the *default* timeline (called `log_time`).
To fix this, add this at the beginning of the main function:
```rust
rec.set_time_seconds("stable_time", 0f64);
```

![logging data - latest at](https://static.rerun.io/0182b4795ca2fed2f2097cfa5f5271115dee0aaf_logging_data8_latest_at.png)

This fix actually introduces yet another very important concept in Rerun: "latest at" semantics.
Notice how, with our latest fix, entities `"dna/structure/left"` & `"dna/structure/right"` have only ever been logged at time zero, and yet they are still visible when querying times far beyond that point.

_Rerun always reasons in terms of "latest" data: for a given entity, it retrieves all of its most recent components at a given time._

## Transforming space

There's only one thing left: our original scene had the abacus rotate along its principal axis.

As was the case with time, (hierarchical) space transformations are first class-citizens in Rerun.
Now it's just a matter of combining the two: we need to log the transform of the scaffolding at each timestamp.

Expand the previous loop to also include:
```rust
for i in 0..400 {
    // ...everything else...
    MsgSender::new("dna/structure")
        .with_component(&[Transform3D::new(transform::RotationAxisAngle::new(
            glam::Vec3::Z,
            rerun::transform::Angle::Radians(time / 4.0 * TAU),
        ))])?
        .send(&recording)?;
}
```

Voila!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>


## Other ways of logging & visualizing data

### Saving & loading to/from RRD files

Sometimes, sending the data over the network is not an option. Maybe you'd like to share the data, attach it to a bug report, etc.

Rerun has you covered:
- Use [`RecordingStream::save`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.save) to stream all logging data to disk.
- Visualize it via `rerun path/to/recording.rrd`

You can also save a recording (or a portion of it) as you're visualizing it, directly from the viewer.

⚠️  [RRD files don't yet handle versioning!](https://github.com/rerun-io/rerun/issues/873) ⚠️

### Closing

This closes our whirlwind tour of Rerun. We've barely scratched the surface of what's possible, but this should have hopefully given you plenty pointers to start experimenting.

As a next step, browse through our [example gallery](/examples) for some more realistic example use-cases, or browse the [Loggable Data Types](../reference/data_types.md) section for more simple examples of how to use the main data types.

