---
title: Send from Rust
order: 3
---

In this section we'll log and visualize our first non-trivial dataset, putting many of Rerun's core concepts and features to use.

In a few lines of code, we'll go from a blank sheet to something you don't see every day: an animated, interactive, DNA-shaped abacus:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

This guide aims to go wide instead of deep.
There are links to other doc pages where you can learn more about specific topics.

At any time, you can checkout the complete code listing for this tutorial [here](https://github.com/rerun-io/rerun/tree/latest/examples/rust/dna/src/main.rs) to better keep track of the overall picture.
To run the example from the repository, run `cargo run -p dna`.

## Prerequisites

We assume you have a working Rust environment and have started a new project with the `rerun` dependency. If not, check out the [installing rust](../../overview/installing-rerun/rust.md).

For this example in particular, we're going to need all of these:

```toml
[dependencies]
rerun = "0.23"
itertools = "0.14"
rand = "0.8"
```

While we're at it, let's get imports out of the way:

```rust
use std::f32::consts::TAU;

use itertools::Itertools as _;
use rand::Rng as _;
use rerun::{
    demo_util::{bounce_lerp, color_spiral},
    external::glam,
};
```

## Starting the Viewer

Just run `rerun` to start the [Rerun Viewer](../../reference/viewer/overview.md). It will wait for your application to log some data to it. This Viewer is in fact a server that's ready to accept data over gRPC (it's listening on `0.0.0.0:9876` by default).

Checkout `rerun --help` for more options.

<picture>
  <img src="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/1200w.png">
</picture>

## Initializing the SDK

To get going we want to create a [`RecordingStream`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html):
We can do all of this with the [`rerun::RecordingStreamBuilder::new`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.new) function which allows us to name the dataset we're working on by setting its [`ApplicationId`](https://docs.rs/rerun/latest/rerun/struct.ApplicationId.html).
We then connect it to the already running Viewer via [`connect_grpc`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.connect_grpc), returning the `RecordingStream` upon success.

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_dna_abacus")
        .connect_grpc()?;

    Ok(())
}
```

Among other things, a stable [`ApplicationId`](https://docs.rs/rerun/latest/rerun/struct.ApplicationId.html) will make it so the [Rerun Viewer](../../reference/viewer/overview.md) retains its UI state across runs for this specific dataset, which will make our lives much easier as we iterate.

Check out the reference to learn more about how Rerun deals with [recordings and datasets](../../concepts/logging-and-ingestion/recordings.md).

## Logging our first points

The core structure of our DNA looking shape can easily be described using two point clouds shaped like spirals.
Add the following to your `main` function:

```rust
const NUM_POINTS: usize = 100;

let (points1, colors1) = color_spiral(NUM_POINTS, 2.0, 0.02, 0.0, 0.1);
let (points2, colors2) = color_spiral(NUM_POINTS, 2.0, 0.02, TAU * 0.5, 0.1);

rec.log(
    "dna/structure/left",
    &rerun::Points3D::new(points1.iter().copied())
        .with_colors(colors1)
        .with_radii([0.08]),
)?;
rec.log(
    "dna/structure/right",
    &rerun::Points3D::new(points2.iter().copied())
        .with_colors(colors2)
        .with_radii([0.08]),
)?;
```

Run your program with `cargo run` and you should now see this scene in the viewer:

<picture>
  <img src="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/1200w.png">
</picture>

_This is a good time to make yourself familiar with the viewer: try interacting with the scene and exploring the different menus._
_Checkout the [Viewer Walkthrough](../configure-the-viewer/navigating-the-viewer.md) and [viewer reference](../../reference/viewer/overview.md) for a complete tour of the viewer's capabilities._

## Under the hood

This tiny snippet of code actually holds much more than meets the eye…

### Archetypes

<!-- TODO(andreas): UPDATE DOC LINKS -->

The easiest way to log geometric primitives is the use the [`RecordingStream::log`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log) method with one of the built-in archetype class, such as [`Points3D`](https://docs.rs/rerun/latest/struct.Points3D.html). Archetypes take care of building batches
of components that are recognized and correctly displayed by the Rerun viewer.

### Components

Under the hood, the Rerun [Rust SDK](https://docs.rs/rerun) logs individual _components_ like positions, colors,
and radii. Archetypes are just one high-level, convenient way of building such collections of components. For advanced use
cases, it's possible to add custom components to archetypes, or even log entirely custom sets of components, bypassing
archetypes altogether.
For more information on how the Rerun data model works, refer to our section on [Entities and Components](../../concepts/logging-and-ingestion/entity-component.md).

Notably, the [`RecordingStream::log`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log) method

<!-- TODO(andreas): UPDATE DOC LINKS -->

will handle any data type that implements the [`AsComponents`](https://docs.rs/rerun/latest/rerun/trait.AsComponents.html) trait, making it easy to add your own data.
For more information on how to supply your own components see [Use custom data](../../howto/logging-and-ingestion/custom-data.md).

### Entities & hierarchies

Note the two strings we're passing in: `"dna/structure/left"` and `"dna/structure/right"`.

These are [_entity paths_](../../concepts/logging-and-ingestion/entity-component.md), which uniquely identify each entity in our scene. Every entity is made up of a path and one or more components.
[Entity paths typically form a hierarchy](../../concepts/logging-and-ingestion/entity-path.md) which plays an important role in how data is visualized and transformed (as we shall soon see).

### Component batches

One final observation: notice how we're logging a whole batch of points and colors all at once here.
[Component batches](../../concepts/logging-and-ingestion/batches.md) are first-class citizens in Rerun and come with all sorts of performance benefits and dedicated features.
You're looking at one of these dedicated features right now in fact: notice how we're only logging a single radius for all these points, yet somehow it applies to all of them. We call this _clamping_.

---

A _lot_ is happening in these two simple function calls.
Good news is: once you've digested all of the above, logging any other entity will simply be more of the same. In fact, let's go ahead and log everything else in the scene now.

## Adding the missing pieces

We can represent the scaffolding using a batch of 3D line segments:

```rust
let lines: Vec<[glam::Vec3; 2]> = points1
    .iter()
    .zip(&points2)
    .map(|(&p1, &p2)| (p1, p2).into())
    .collect_vec();

rec.log(
    "dna/structure/scaffolding",
    &rerun::LineStrips3D::new(lines.iter().cloned())
        .with_colors([rerun::Color::from_rgb(128, 128, 128)]),
)?;
```

Which only leaves the beads:

```rust
let mut rng = rand::rng();
let offsets = (0..NUM_POINTS).map(|_| rng.random::<f32>()).collect_vec();

let beads = lines
    .iter()
    .zip(&offsets)
    .map(|(&[p1, p2], &offset)| bounce_lerp(p1, p2, offset))
    .collect_vec();
let colors = offsets
    .iter()
    .map(|&offset| bounce_lerp(80.0, 230.0, offset * 2.0) as u8)
    .map(|c| rerun::Color::from_rgb(c, c, c))
    .collect_vec();

rec.log(
    "dna/structure/scaffolding/beads",
    &rerun::Points3D::new(beads)
        .with_colors(colors)
        .with_radii([0.06]),
)?;
```

Once again, although we are getting fancier and fancier with our iterator mappings, there is nothing new here: it's all about populating archetypes and feeding them to the Rerun API.

<picture>
  <img src="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/1200w.png">
</picture>

## Animating the beads

### Introducing time

Up until this point, we've completely set aside one of the core concepts of Rerun: [Time and Timelines](../../concepts/logging-and-ingestion/timelines.md).

Even so, if you look at your [Timeline View](../../reference/viewer/timeline.md) right now, you'll notice that Rerun has kept track of time on your behalf anyway by memorizing when each log call occurred.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data6_timeline/f22a3c92ae4f9f3a04901ec907a245e03e9dad68/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data6_timeline/f22a3c92ae4f9f3a04901ec907a245e03e9dad68/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data6_timeline/f22a3c92ae4f9f3a04901ec907a245e03e9dad68/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data6_timeline/f22a3c92ae4f9f3a04901ec907a245e03e9dad68/1200w.png">
  <img src="https://static.rerun.io/logging_data6_timeline/f22a3c92ae4f9f3a04901ec907a245e03e9dad68/full.png" alt="screenshot of the beads with the timeline">
</picture>

Unfortunately, the logging time isn't particularly helpful to us in this case: we can't have our beads animate depending on the logging time, else they would move at different speeds depending on the performance of the logging process!
For that, we need to introduce our own custom timeline that uses a deterministic clock which we control.

Rerun has rich support for time: whether you want concurrent or disjoint timelines, out-of-order insertions or even data that lives _outside_ the timeline(s). You will find a lot of flexibility in there.

Let's add our custom timeline:

```rust
for i in 0..400 {
    let time = i as f32 * 0.01;

    rec.set_duration_secs("stable_time", time);

    let times = offsets.iter().map(|offset| time + offset).collect_vec();
    let beads = lines
        .iter()
        .zip(&times)
        .map(|(&[p1, p2], &time)| bounce_lerp(p1, p2, time))
        .collect_vec();
    let colors = times
        .iter()
        .map(|time| bounce_lerp(80.0, 230.0, time * 2.0) as u8)
        .map(|c| rerun::Color::from_rgb(c, c, c))
        .collect_vec();

    rec.log(
        "dna/structure/scaffolding/beads",
        &rerun::Points3D::new(beads)
            .with_colors(colors)
            .with_radii([0.06]),
    )?;
}
```

First we use [`RecordingStream::set_time_seconds`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.set_time_seconds) to declare our own custom `Timeline` and set the current timestamp.
You can add as many timelines and timestamps as you want when logging data.

⚠️ If you run this code as is, the result will be.. surprising: the beads are animating as expected, but everything we've logged until that point is gone! ⚠️

![logging data - wat](https://static.rerun.io/a396c8aae1cbd717a3f35472594f789e4829b1ae_logging_data7_wat.png)

Enter…

### Latest-at semantics

That's because the Rerun Viewer has switched to displaying your custom timeline by default, but the original data was only logged to the _default_ timeline (called `log_time`).
To fix this, add this at the beginning of the main function:

```rust
rec.set_duration_secs("stable_time", 0.0);
```

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data8_latest_at/295492c6cbc68bff129fbe80bf861793b73b0d29/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data8_latest_at/295492c6cbc68bff129fbe80bf861793b73b0d29/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data8_latest_at/295492c6cbc68bff129fbe80bf861793b73b0d29/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data8_latest_at/295492c6cbc68bff129fbe80bf861793b73b0d29/1200w.png">
  <img src="https://static.rerun.io/logging_data8_latest_at/295492c6cbc68bff129fbe80bf861793b73b0d29/full.png" alt="screenshot after using latest-at">
</picture>

This fix actually introduces yet another very important concept in Rerun: "latest-at" semantics.
Notice how entities `"dna/structure/left"` & `"dna/structure/right"` have only ever been logged at time zero, and yet they are still visible when querying times far beyond that point.

_Rerun always reasons in terms of "latest" data: for a given entity, it retrieves all of its most recent components at a given time._

## Transforming space

There's only one thing left: our original scene had the abacus rotate along its principal axis.

As was the case with time, (hierarchical) space transformations are first class-citizens in Rerun.
Now it's just a matter of combining the two: we need to log the transform of the scaffolding at each timestamp.

Either expand the previous loop to include logging transforms or
simply add a second loop like this:

```rust
for i in 0..400 {
    let time = i as f32 * 0.01;

    rec.set_duration_secs("stable_time", time);

    rec.log(
        "dna/structure",
        &rerun::archetypes::Transform3D::from_rotation(rerun::RotationAxisAngle::new(
            glam::Vec3::Z,
            rerun::Angle::from_radians(time / 4.0 * TAU),
        )),
    )?;
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

-   Use [`RecordingStream::save`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.save) to stream all logging data to disk.
-   Visualize it via `rerun path/to/recording.rrd`

You can also save a recording (or a portion of it) as you're visualizing it, directly from the viewer.

### RRD file backwards compatibility

RRD files saved with Rerun 0.23 or later can be opened with a newer Rerun version.
For more details and potential limitations, please refer to [our blog post](https://rerun.io/blog/release-0.23).

⚠️ At the moment, we only guarantee compatibility across adjacent minor versions (e.g. Rerun 0.24 can open RRDs from 0.23).

### Spawning the Viewer from your process

If the Rerun Viewer is [installed](../../overview/installing-rerun.md) and available in your `PATH`, you can use [`RecordingStream::spawn`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.spawn) to automatically start a Viewer in a new process and connect to it over gRPC.
If an external Viewer was already running, `spawn` will connect to that one instead of spawning a new one.

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_dna_abacus")
        .spawn()?;

    // … log data to `rec` …

    Ok(())
}
```

Alternatively, you can use [`rerun::native_viewer::show`](https://docs.rs/rerun/latest/rerun/native_viewer/fn.show.html) to start a Viewer on the main thread (for platform-compatibility reasons) and feed it data from memory.
This requires the `native_viewer` feature to be enabled in `Cargo.toml`:

```toml
rerun = { version = "0.9", features = ["native_viewer"] }
```

Doing so means you're building the Rerun Viewer itself as part of your project, meaning compilation will take a bit longer the first time.

Unlike `spawn` however, this expects a complete recording instead of being fed in real-time:

```rust
let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_dna_abacus").memory()?;

// … log data to `rec` …

// Blocks until the viewer is closed.
// For more customizations, refer to `re_viewer::run_native_app`.
rerun::show(
    // Show has to be called on the main thread.
    rerun::MainThreadToken::i_promise_i_am_on_the_main_thread(),
    storage.take(),
)?;
```

The Viewer will block the main thread until it is closed.

### Closing

This closes our whirlwind tour of Rerun. We've barely scratched the surface of what's possible, but this should have hopefully given you plenty pointers to start experimenting.

As a next step, browse through our [example gallery](/examples) for some more realistic example use-cases, browse the [Types](../../reference/types.md) section for more simple examples of how to use the main data types, or dig deeper into [querying your logged data](../data-out.md).
