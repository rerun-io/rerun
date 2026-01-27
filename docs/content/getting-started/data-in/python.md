---
title: Send from Python
order: 2
---

In this section we'll log and visualize our first non-trivial dataset, putting many of Rerun's core concepts and features to use.

In a few lines of code, we'll go from a blank sheet to something you don't see every day: an animated, interactive, DNA-shaped abacus:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

This guide aims to go wide instead of deep.
There are links to other doc pages where you can learn more about specific topics.

At any time, you can checkout the complete code listing for this tutorial [here](https://github.com/rerun-io/rerun/tree/latest/examples/python/dna/dna.py) to better keep track of the overall picture.

## Prerequisites

We assume you have working Python and `rerun-sdk` installations. If not, check out [installing python](../../overview/installing-rerun/python.md).

## Initializing the SDK

Start by opening your editor of choice and creating a new file called `dna_example.py`.

The first thing we need to do is to import `rerun` and initialize the SDK by calling [`rr.init`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.init). This init call is required prior to using any of the global
logging calls, and allows us to name our recording using an `ApplicationId`.

We also import some other utilities we will use later in the example.

```python
import rerun as rr

from math import tau
import numpy as np
from rerun.utilities import build_color_spiral
from rerun.utilities import bounce_lerp

rr.init("rerun_example_dna_abacus")
```

A stable [`ApplicationId`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.init) will make it so the [Rerun Viewer](../../reference/viewer/overview.md) retains its UI state across runs for this specific dataset, which will make our lives much easier as we iterate.

Check out the reference to learn more about how Rerun deals with [applications and recordings](../../concepts/logging-and-ingestion/recordings.md).

## Starting the Viewer

Next up, we want to spawn the [Rerun Viewer](../../reference/viewer/overview.md) itself.

To do this, you can add the line:

```python
rr.spawn()
```

Now you can run your application just as you would any other Python script:

```
(venv) $ python dna_example.py
```

And with that, we're ready to start sending out data:

<picture>
  <img src="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/logging_data2_empty/2915f4ef35db229caee6e5cb380b47aa4ecc0b33/1200w.png">
</picture>

By default, the SDK will start a Viewer in another process and automatically pipe the data through.
There are other means of sending data to a Viewer as we'll see at the end of this section, but for now this default will work great as we experiment.

## Logging our first points

The core structure of our DNA looking shape can easily be described using two point clouds shaped like spirals.
Add the following to your file:

```python
NUM_POINTS = 100

# Points and colors are both np.array((NUM_POINTS, 3))
points1, colors1 = build_color_spiral(NUM_POINTS)
points2, colors2 = build_color_spiral(NUM_POINTS, angular_offset=tau*0.5)

rr.log("dna/structure/left", rr.Points3D(points1, colors=colors1, radii=0.08))
rr.log("dna/structure/right", rr.Points3D(points2, colors=colors2, radii=0.08))
```

Run your script once again and you should now see this scene in the viewer.
Note that if the Viewer was still running, Rerun will simply connect to this existing session and replace the data with this new [_recording_](../../concepts/logging-and-ingestion/recordings.md).

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

The easiest way to log geometric primitives is the use the [`rr.log`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log) function with one of the built-in archetype classes, such as [`rr.Points3D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Points3D). Archetypes take care of building batches
of components that are recognized and correctly displayed by the Rerun viewer.

### Components

Under the hood, the Rerun [Python SDK](https://ref.rerun.io/docs/python) logs individual _components_ like positions, colors,
and radii. Archetypes are just one high-level, convenient way of building such collections of components. For advanced use
cases, it's possible to add custom components to archetypes, or even log entirely custom sets of components, bypassing
archetypes altogether.

For more information on how the Rerun data model works, refer to our section on [Entities and Components](../../concepts/logging-and-ingestion/entity-component.md).

Our [Python SDK](https://ref.rerun.io/docs/python) integrates with the rest of the Python ecosystem: the points and colors returned by [`build_color_spiral`](https://ref.rerun.io/docs/python/stable/common/demo_utilities/#rerun.utilities.build_color_spiral) in this example are vanilla `numpy` arrays.
Rerun takes care of mapping those arrays to actual Rerun components depending on the context (e.g. we're calling [`rr.Points3D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Points3D) in this case).

### Entities & hierarchies

Note the two strings we're passing in: `"dna/structure/left"` & `"dna/structure/right"`.

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

We can represent the scaffolding using a batch of 3D line strips:

```python
rr.log(
    "dna/structure/scaffolding",
    rr.LineStrips3D(np.stack((points1, points2), axis=1), colors=[128, 128, 128])
)
```

Which only leaves the beads:

```python
offsets = np.random.rand(NUM_POINTS)
beads = [bounce_lerp(points1[n], points2[n], offsets[n]) for n in range(NUM_POINTS)]
colors = [[int(bounce_lerp(80, 230, offsets[n] * 2))] for n in range(NUM_POINTS)]
rr.log(
    "dna/structure/scaffolding/beads",
    rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1)),
)
```

Once again, although we are getting fancier and fancier with our [`numpy` incantations](https://ref.rerun.io/docs/python/stable/common/demo_utilities/#rerun.utilities.util.bounce_lerp),
there is nothing new here: it's all about building out `numpy` arrays and feeding them to the Rerun API.

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

```python
time_offsets = np.random.rand(NUM_POINTS)

for i in range(400):
    time = i * 0.01
    rr.set_time("stable_time", duration=time)

    times = np.repeat(time, NUM_POINTS) + time_offsets
    beads = [bounce_lerp(points1[n], points2[n], times[n]) for n in range(NUM_POINTS)]
    colors = [[int(bounce_lerp(80, 230, times[n] * 2))] for n in range(NUM_POINTS)]
    rr.log(
        "dna/structure/scaffolding/beads",
        rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1)),
    )
```

A call to [`set_time`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time) will create our new `Timeline` and make sure that any logging calls that follow gets assigned that time.

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
To fix this, go back to the top of the file and add:

```python
rr.spawn()
rr.set_time("stable_time", duration=0)
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

```python
for i in range(400):
    time = i * 0.01
    rr.set_time("stable_time", duration=time)
    rr.log(
        "dna/structure",
        rr.Transform3D(rotation=rr.RotationAxisAngle(axis=[0, 0, 1], radians=time / 4.0 * tau)),
    )
```

Voila!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

## Other ways of logging & visualizing data

[`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn) is great when you're experimenting on a single machine like we did in this tutorial, but what if the logging happens on, for example, a headless computer?

Rerun offers several solutions for such use cases.

### Logging data over the network

At any time, you can start a Rerun Viewer by running `rerun`. This Viewer is in fact a server that's ready to accept data over gRPC (it's listening on `0.0.0.0:9876` by default).

On the logger side, simply use [`rr.connect_grpc`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.connect_grpc) instead of [`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn) to start sending the data over to any gRPC address.

Checkout `rerun --help` for more options.

### Saving & loading to/from RRD files

Sometimes, sending the data over the network is not an option. Maybe you'd like to share the data, attach it to a bug report, etc.

Rerun has you covered:

-   Use [`rr.save`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.save) to stream all logged data to disk.
-   View it with `rerun path/to/recording.rrd`

You can also save a recording (or a portion of it) as you're visualizing it, directly from the viewer.

### RRD file backwards compatibility

RRD files saved with Rerun 0.23 or later can be opened with a newer Rerun version.
For more details and potential limitations, please refer to [our blog post](https://rerun.io/blog/release-0.23).

⚠️ At the moment, we only guarantee compatibility across adjacent minor versions (e.g. Rerun 0.24 can open RRDs from 0.23).

## Closing

This closes our whirlwind tour of logging with Rerun. We've barely scratched the surface of what's possible, but this should have hopefully given you plenty pointers to start experimenting.

As a next step, browse through our [example gallery](/examples) for some more realistic example use-cases, browse the [Types](../../reference/types.md) section for more simple examples of how to use the main datatypes, or dig deeper into [querying your logged data](../data-out.md).
