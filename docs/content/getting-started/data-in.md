---
title: Log and Ingest
order: 400
---

In this section we'll log and visualize our first non-trivial dataset, putting many of Rerun's core concepts and features to use.

In a few lines of code, we'll go from a blank sheet to something you don't see every day: an animated, interactive, DNA-shaped abacus:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

This guide aims to go wide instead of deep.
There are links to other doc pages where you can learn more about specific topics.

The complete code listings for this tutorial live alongside the Rerun source tree:
[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/dna/dna.py),
[Rust](https://github.com/rerun-io/rerun/tree/latest/examples/rust/dna/src/main.rs),
[C++](https://github.com/rerun-io/rerun/tree/latest/examples/cpp/dna/main.cpp).

## Prerequisites

Before starting, make sure you've [installed the SDK](./install-rerun.md) and [set up a project](./project-setup.md) for your language of choice.

## Initializing the SDK

Create a new file (or project), import the relevant utilities from your language's SDK, and initialize a recording. Initialization names the recording with a stable [`ApplicationId`](../concepts/logging-and-ingestion/recordings.md), then spawns a [Rerun Viewer](../reference/viewer/overview.md) and connects the recording to it:

snippet: tutorials/dna[imports]

snippet: tutorials/dna[init]

A stable `ApplicationId` will make the Viewer retain its UI state across runs for this specific dataset, which makes our lives much easier as we iterate.

By default, `spawn` will start a Viewer in another process and automatically pipe the data through. There are other ways to send data to a Viewer (covered at the end of this section), but the spawn default works great as we experiment.

<picture>
  <img src="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/1200w.png">
</picture>

## Logging our first points

The core structure of our DNA-looking shape can easily be described using two point clouds shaped like spirals:

snippet: tutorials/dna[first_points]

Run your program and you should now see this scene in the viewer.
If the Viewer was still running, Rerun will simply connect to this existing session and replace the data with this new [_recording_](../concepts/logging-and-ingestion/recordings.md).

<picture>
  <img src="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data3_first_points/95c9c556160159eb2e47fb160ced89c899f2fcef/1200w.png">
</picture>

_This is a good time to make yourself familiar with the viewer: try interacting with the scene and exploring the different menus._
_Checkout the [Viewer Walkthrough](configure-the-viewer/navigating-the-viewer.md) and [viewer reference](../reference/viewer/overview.md) for a complete tour of the viewer's capabilities._

## Under the hood

This tiny snippet of code actually holds much more than meets the eye…

### Archetypes

The easiest way to log geometric primitives is to use the SDK's `log` method with one of the built-in archetype classes (such as `Points3D` here). Archetypes take care of building batches of components that are recognized and correctly displayed by the Rerun viewer.

### Components

Under the hood, the Rerun SDK logs individual _components_ like positions, colors, and radii. Archetypes are just one high-level, convenient way of building such collections of components. For advanced use cases, it's possible to add custom components to archetypes, or even log entirely custom sets of components, bypassing archetypes altogether.

For more information on how the Rerun data model works, refer to our section on [Entities and Components](../concepts/logging-and-ingestion/entity-component.md). For supplying your own components, see [Use custom data](../howto/logging-and-ingestion/custom-data.md).

### Entities & hierarchies

Note the two strings we're passing in: `"dna/structure/left"` & `"dna/structure/right"`.

These are [_entity paths_](../concepts/logging-and-ingestion/entity-component.md), which uniquely identify each entity in our scene. Every entity is made up of a path and one or more components.
[Entity paths typically form a hierarchy](../concepts/logging-and-ingestion/entity-path.md) which plays an important role in how data is visualized and transformed (as we shall soon see).

### Component batches

One final observation: notice how we're logging a whole batch of points and colors all at once.
[Component batches](../concepts/logging-and-ingestion/batches.md) are first-class citizens in Rerun and come with all sorts of performance benefits and dedicated features.
You're looking at one of these dedicated features right now: notice how we're only logging a single radius for all these points, yet somehow it applies to all of them. We call this _clamping_.

---

A _lot_ is happening in these two simple function calls.
Good news is: once you've digested all of the above, logging any other entity will simply be more of the same. In fact, let's go ahead and log everything else in the scene now.

## Adding the missing pieces

We can represent the scaffolding using a batch of 3D line strips:

snippet: tutorials/dna[scaffolding]

Which only leaves the beads:

snippet: tutorials/dna[beads]

Once again, although we are getting fancier with our array manipulations, there is nothing new here: it's all about populating archetypes and feeding them to the Rerun API.

<picture>
  <img src="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data5_beads/53afa6ca96259c4451a8b7722a8856252c2fdba6/1200w.png">
</picture>

## Animating the beads

### Introducing time

Up until this point, we've completely set aside one of the core concepts of Rerun: [Time and Timelines](../concepts/logging-and-ingestion/timelines.md).

Even so, if you look at your [Timeline View](../reference/viewer/timeline.md) right now, you'll notice that Rerun has kept track of time on your behalf anyway by memorizing when each log call occurred.

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

Replace the section that logs the beads with a loop that logs them at different timestamps:

snippet: tutorials/dna[time_loop]

A call to `set_time` (or `set_duration_secs` in Rust / `set_time_duration` in C++) creates our new `Timeline` and makes sure that any logging calls that follow get assigned that time.
You can add as many timelines and timestamps as you want when logging data.

⚠️ If you run this code as is, the result will be… surprising: the beads are animating as expected, but everything we've logged until that point is gone! ⚠️

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data7_wat/2a3b65f4a0e1e948184d85bab497e4bffdda0b7e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data7_wat/2a3b65f4a0e1e948184d85bab497e4bffdda0b7e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data7_wat/2a3b65f4a0e1e948184d85bab497e4bffdda0b7e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data7_wat/2a3b65f4a0e1e948184d85bab497e4bffdda0b7e/1200w.png">
  <img src="https://static.rerun.io/logging_data7_wat/2a3b65f4a0e1e948184d85bab497e4bffdda0b7e/full.png" alt="screenshot of the surprising situation">
</picture>

Enter…

### Latest-at semantics

That's because the Rerun Viewer has switched to displaying your custom timeline by default, but the original data was only logged to the _default_ timeline (called `log_time`).
To fix this, set the custom timeline to time zero before logging the original structure:

snippet: tutorials/dna[latest_at_fix]

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

As was the case with time, (hierarchical) space transformations are first-class citizens in Rerun.
Now it's just a matter of combining the two: we need to log the transform of the scaffolding at each timestamp.

Either expand the previous loop to include logging transforms or simply add a second loop like this:

snippet: tutorials/dna[transform_loop]

Voila!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

## Other ways of logging & visualizing data

`spawn` is great when you're experimenting on a single machine like we did in this tutorial, but what if the logging happens on, for example, a headless computer?

Rerun offers several solutions for such use cases.

### Logging data over the network

At any time, you can start a Rerun Viewer by running `rerun`. This Viewer is in fact a server that's ready to accept data over gRPC (it's listening on `0.0.0.0:9876` by default).

On the logger side, replace the `spawn` call from above with a `connect_grpc` call to send data to any gRPC address:

snippet: tutorials/dna_connect_grpc

Run `rerun --help` for more options.

### Saving & loading to/from RRD files

Sometimes, sending data over the network is not an option. Maybe you'd like to share the data, attach it to a bug report, etc.

Rerun has you covered: each SDK exposes a `save` method (Python: [`rr.save`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.save), Rust: [`RecordingStream::save`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.save), C++: [`RecordingStream::save`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a555a7940a076c93d951de5b139d14918)) that streams all logged data to disk. View the resulting file with `rerun path/to/recording.rrd`.

You can also save a recording (or a portion of it) as you're visualizing it, directly from the viewer.

### RRD file backwards compatibility

RRD files saved with Rerun 0.23 or later can be opened with a newer Rerun version.
For more details and potential limitations, please refer to [our blog post](https://rerun.io/blog/release-0.23).

⚠️ At the moment, we only guarantee compatibility across adjacent minor versions (e.g. Rerun 0.24 can open RRDs from 0.23).

### Rust-only: showing the Viewer in-process

The Rust SDK can host the Viewer directly inside your application via [`rerun::native_viewer::show`](https://docs.rs/rerun/latest/rerun/native_viewer/fn.show.html), which expects a complete recording from memory rather than a live stream. This requires enabling the `native_viewer` feature in `Cargo.toml`. The Viewer blocks the main thread until closed; see the Rust API docs for details.

## Closing

This closes our whirlwind tour of logging with Rerun. We've barely scratched the surface of what's possible, but this should have hopefully given you plenty of pointers to start experimenting.

As a next step, browse through our [example gallery](https://rerun.io/examples) for some more realistic example use-cases, browse the [Types](../reference/types.md) section for more simple examples of how to use the main datatypes, or dig deeper into [querying your logged data](data-out.md).

## Opening files

You can also open existing files (RRD, MCAP, images, video, point clouds, etc.) directly with the Viewer — see [Opening files](data-in/open-any-file.md).
