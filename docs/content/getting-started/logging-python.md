---
title: Logging Data in Python
order: 4
---

In this section we'll log and visualize our first non-trivial dataset, putting many of Rerun's core concepts and features to use.

In a few lines of code, we'll go from a blank sheet to something you don't see everyday: an animated, interactive, DNA-shaped abacus:
<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

This guide aims to go wide instead of deep.
There are links to other doc pages where you can learn more about specific topics.

At any time, you can checkout the complete code listing for this tutorial [here](https://github.com/rerun-io/rerun/tree/latest/examples/python/dna/main.py) to better keep track of the overall picture.

## Prerequisites

We assume you have working Python and `rerun-sdk` installations. If not, check out the [setup page](python.md).

For this tutorial you will also need to `pip install numpy scipy`.

## Initializing the SDK

Start by opening your editor of choice and creating a new file called `dna_example.py`.

The first thing we need to do is to import `rerun` and initialize the SDK by calling [`rr.init`](https://ref.rerun.io/docs/python/latest/common/initialization/#rerun.init). This init call is required prior to using any of the global
logging calls, and allows us to name our recording using an `ApplicationId`.

```python
import rerun as rr

rr.init("rerun-example-DNA Abacus")
```

Among other things, a stable [`ApplicationId`](https://ref.rerun.io/docs/python/latest/common/initialization/#rerun.init) will make it so the [Rerun Viewer](../reference/viewer/overview.md) retains its UI state across runs for this specific dataset, which will make our lives much easier as we iterate.

Check out the reference to learn more about how Rerun deals with [applications and recordings](../concepts/apps-and-recordings.md).

## Starting the Viewer

Next up, we want to spawn the [Rerun Viewer](../reference/viewer/overview.md) itself.

To do this, you can add the line:
```python
rr.spawn()
```

Now you can run your application just as you would any other python script:
```
(venv) $ python dna_example.py
```

And with that, we're ready to start sending out data:

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/b509f0c8ba4a46ed8ffd68bea4ade384525a41a1_logging_data2_waiting_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/89b4674ee51f3f5365bad2347bc3f7319ff8de0f_logging_data2_waiting_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/349e38deb9307100471b66becd6fab5d09dae80f_logging_data2_waiting_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/4820da379135db095f0d160550245114e6257400_logging_data2_waiting_1200w.png">
  <img src="https://static.rerun.io/a45b879f74828ff22d0767c7892bffb17a0ac25f_logging_data2_waiting_full.png" alt="screenshot of the waiting screen">
</picture>



By default, the SDK will start a viewer in another process and automatically pipe the data through.
There are other means of sending data to a viewer as we'll see at the end of this section, but for now this default will work great as we experiment.

---
The following sections will require importing a few different things to your script.
We will do so incrementally, but if you just want to update your imports once and call it a day, feel free to add the following to the top of your script:
```python
from math import tau
import numpy as np
from rerun_demo.data import build_color_spiral
from rerun_demo.util import bounce_lerp, interleave
from scipy.spatial.transform import Rotation
```
---

## Logging our first points

The core structure of our DNA looking shape can easily be described using two point clouds shaped like spirals.
Add the following to your file:
```python
# new imports
from rerun_demo.data import build_color_spiral
from math import tau

NUM_POINTS = 100

# points and colors are both np.array((NUM_POINTS, 3))
points1, colors1 = build_color_spiral(NUM_POINTS)
points2, colors2 = build_color_spiral(NUM_POINTS, angular_offset=tau*0.5)

rr.log_points("dna/structure/left", points1, colors=colors1, radii=0.08)
rr.log_points("dna/structure/right", points2, colors=colors2, radii=0.08)
```

Run your script once again and you should now see this scene in the viewer.
Note that if the viewer was still running, Rerun will simply connect to this existing session and replace the data with this new [_recording_](../concepts/apps-and-recordings.md).

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/5305cec597b9957036291198402d5afbbcce218e_logging_data3_first_points_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/5fa88ed6192920f6162b7080928aa538ae2025dd_logging_data3_first_points_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/9a87ed7e20b72902c6506a87fda3d62f716eed88_logging_data3_first_points_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/b32a40031e47a4aa3b1669bef53048765c1b7b10_logging_data3_first_points_1200w.png">
  <img src="https://static.rerun.io/0e0fada083d59cf14ad20ed0ee50ef88a9550d82_logging_data3_first_points_full.png" alt="screenshot after logging the first points">
</picture>


_This is a good time to make yourself familiar with the viewer: try interacting with the scene and exploring the different menus._
_Checkout the [Viewer Walkthrough](viewer-walkthrough.md) and [viewer reference](../reference/viewer/overview.md) for a complete tour of the viewer's capabilities._

### Under the hood

This tiny snippet of code actually holds much more than meets the eye...

`Components`

Although the Rerun [Python SDK](https://ref.rerun.io/docs/python) exposes concepts related to logging primitives such as points, and lines, under the hood these primitives are made up of individual components like positions, colors, and radii. For more information on how the rerun data model works, refer to our section on [entities and components](../concepts/entity-component.md).

Our [Python SDK](https://ref.rerun.io/docs/python) integrates with the rest of the Python ecosystem: the points and colors returned by [`build_color_spiral`](https://ref.rerun.io/docs/python/latest/package/rerun_demo/data/#rerun_demo.data.build_color_spiral) in this example are vanilla `numpy` arrays.
Rerun takes care of mapping those arrays to actual Rerun components depending on the context (e.g. we're calling [`log_points`](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_points) in this case).

`Entities & hierarchies`

Note the two strings we're passing in: `"dna/structure/left"` & `"dna/structure/right"`.

These are [Entity Paths](../concepts/entity-component.md), which uniquely identify each Entity in our scene. Every Entity is made up of a path and one or more Components.
[Entity paths typically form a hierarchy](../concepts/entity-path.md) which plays an important role in how data is visualized and transformed (as we shall soon see).

`Batches`

One final observation: notice how we're logging a whole batch of points and colors all at once here.
[Batches of data](../concepts/batches.md) are first-class citizens in Rerun and come with all sorts of performance benefits and dedicated features.
You're looking at one of these dedicated features right now in fact: notice how we're only logging a single radius for all these points, yet somehow it applies to all of them.

---

A _lot_ is happening in these two simple function calls.
Good news is: once you've digested all of the above, logging any other Component will simply be more of the same. In fact, let's go ahead and log everything else in the scene now.

## Adding the missing pieces

We can represent the scaffolding using a batch of 3D line segments:
```python
# new imports
from rerun_demo.util import interleave

points = interleave(points1, points2)
rr.log_line_segments("dna/structure/scaffolding", points, color=[128, 128, 128])
```

Which only leaves the beads:
```python
# new imports
import numpy as np
from rerun_demo.util import bounce_lerp

offsets = np.random.rand(NUM_POINTS)
beads = [bounce_lerp(points1[n], points2[n], offsets[n]) for n in range(NUM_POINTS)]
colors = [[int(bounce_lerp(80, 230, offsets[n] * 2))] for n in range(NUM_POINTS)]
rr.log_points("dna/structure/scaffolding/beads", beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1))
```

Once again, although we are getting fancier and fancier with our [`numpy` incantations](https://ref.rerun.io/docs/python/latest/package/rerun_demo/util/#rerun_demo.util.bounce_lerp),
there is nothing new here: it's all about building out `numpy` arrays and feeding them to the Rerun API.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/5ad83403acc560dac2bd85211419482c60019783_logging_data5_beads_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/cdec3af724a3ceff147c1c6fd6cc9dbc51a2204d_logging_data5_beads_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/cb272220c85e3376c75fd6da7f18462b920e172a_logging_data5_beads_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/de68973ad6420112eba8b91217d9f4713d5d9a02_logging_data5_beads_1200w.png">
  <img src="https://static.rerun.io/7825d736cb8b7dab27f27aa7611c4bac24f23433_logging_data5_beads_full.png" alt="screenshot after logging beads">
</picture>


## Animating the beads

### Introducing Time

Up until this point, we've completely set aside one of the core concepts of Rerun: [Time and Timelines](../concepts/timelines.md).

Even so, if you look at your [Timeline View](../reference/viewer/timeline.md) right now, you'll notice that Rerun has kept track of time on your behalf anyways by memorizing when each log call occurred.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/720e5ba4d1881f88e1f0dd3342c52f3b472b70c6_logging_data6_timeline_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/96bdee91b139b19023524bc6954798005a24f298_logging_data6_timeline_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/c824ea981c457ee11c03b10645245b51e75de428_logging_data6_timeline_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/d8dcfc5d19865d0c50f43d377fbbcad027d44bf3_logging_data6_timeline_1200w.png">
  <img src="https://static.rerun.io/37fbc6fcbe8e89b2267057e3eae2e56b8d9ddf42_logging_data6_timeline_full.png" alt="screenshot of the beads with the timeline">
</picture>


Unfortunately, the logging time isn't particularly helpful to us in this case: we can't have our beads animate depending on the logging time, else they would move at different speeds depending on the performance of the logging process!
For that, we need to introduce our own custom timeline that uses a deterministic clock which we control.

Rerun has rich support for time: whether you want concurrent or disjoint timelines, out-of-order insertions or even data that lives _outside_ of the timeline(s)… you'll find a lot of flexibility in there.

Let's add our custom timeline:
```python
# new imports
from rerun_demo.util import bounce_lerp

time_offsets = np.random.rand(NUM_POINTS)
for i in range(400):
    time = i * 0.01
    rr.set_time_seconds("stable_time", time)

    times = np.repeat(time, NUM_POINTS) + time_offsets
    beads = [bounce_lerp(points1[n], points2[n], times[n]) for n in range(NUM_POINTS)]
    colors = [[int(bounce_lerp(80, 230, times[n] * 2))] for n in range(NUM_POINTS)]
    rr.log_points("dna/structure/scaffolding/beads", beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1))
```

A call to [`set_time_seconds`](https://ref.rerun.io/docs/python/latest/common/time/#rerun.set_time_seconds) will create our new `Timeline` and make sure that any logging calls that follow gets assigned that time.

⚠️  If you run this code as is, the result will be.. surprising: the beads are animating as expected, but everything we've logged until that point is gone! ⚠️

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/af29a2c3ed2c7a829cd91c2ad11d38f4418b9b23_logging_data7_wat_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/a46e9b2e7cc03366712fc0d3e3bc5bcb1efb406c_logging_data7_wat_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/398fb5f7164b6ccf010e4af8b5c761818bf015a1_logging_data7_wat_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/263d82b50e184ecb4d83350a1ef8b89b5a4d7b5f_logging_data7_wat_1200w.png">
  <img src="https://static.rerun.io/74239084e9e45d2a0187b5e5e944063e3c8df5fa_logging_data7_wat_full.png" alt="screenshot of the surprising situation">
</picture>


Enter...

### Latest At semantics

That's because the Rerun Viewer has switched to displaying your custom timeline by default, but the original data was only logged to the *default* timeline (called `log_time`).
To fix this, go back to the top of the file and add:
```python
rr.spawn()
rr.set_time_seconds("stable_time", 0)
```

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/6c6918d4e1c33dc321d15e8746753ed2b1ca2037_logging_data8_latest_at_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/73755db0243987f750f620c34d2b5f67e47c6ab4_logging_data8_latest_at_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/72d2625d37ed6b149a4f5c016768d300c36a9bee_logging_data8_latest_at_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/a9559392295bc0bfc75da81badae2ae053f707e7_logging_data8_latest_at_1200w.png">
  <img src="https://static.rerun.io/0a7c2f339aaff03c3ccf82d5686f3af2c19a00a7_logging_data8_latest_at_full.png" alt="screenshot after using latest at">
</picture>


This fix actually introduces yet another very important concept in Rerun: "latest at" semantics.
Notice how entities `"dna/structure/left"` & `"dna/structure/right"` have only ever been logged at time zero, and yet they are still visible when querying times far beyond that point.

_Rerun always reasons in terms of "latest" data: for a given entity, it retrieves all of its most recent components at a given time._

## Transforming space

There's only one thing left: our original scene had the abacus rotate along its principal axis.

As was the case with time, (hierarchical) space transformations are first class-citizens in Rerun.
Now it's just a matter of combining the two: we need to log the transform of the scaffolding at each timestamp.

Either expand the previous loop to include logging transforms or
simply add a second loop like this:
```python
# new imports
from scipy.spatial.transform import Rotation

for i in range(400):
    time = i * 0.01
    rr.set_time_seconds("stable_time", time)
    rr.log_transform3d(
        "dna/structure",
        rr.RotationAxisAngle(axis=[0, 0, 1], radians=time / 4.0 * tau),
    )
```

Voila!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>


## Other ways of logging & visualizing data

[`rr.spawn`](https://ref.rerun.io/docs/python/latest/package/rerun/__init__/#rerun.spawn) is great when you're experimenting on a single machine like we did in this tutorial, but what if the process that's doing the logging doesn't have a graphical interface to begin with?

Rerun offers several solutions for these use cases.

### Logging data over the network

At any time, you can start a Rerun Viewer by running `rerun`. This viewer is in fact a server that's ready to accept data over TCP (it's listening on `0.0.0.0:9876` by default).

On the logger side, simply use [`rr.connect`](https://ref.rerun.io/docs/python/latest/common/initialization/#rerun.connect) instead of [`rr.spawn`](https://ref.rerun.io/docs/python/latest/common/initialization/#rerun.spawn) to start sending the data over to any TCP address.

Checkout `rerun --help` for more options.

### Saving & loading to/from RRD files

Sometimes, sending the data over the network is not an option. Maybe you'd like to share the data, attach it to a bug report, etc.

Rerun has you covered:
- Use [`rr.save`](https://ref.rerun.io/docs/python/latest/package/rerun/__init__/#rerun.save) to stream all logged data to disk.
- View it with `rerun path/to/recording.rrd`

You can also save a recording (or a portion of it) as you're visualizing it, directly from the viewer.

⚠️  [RRD files don't yet handle versioning!](https://github.com/rerun-io/rerun/issues/873) ⚠️

### Closing

This closes our whirlwind tour of Rerun. We've barely scratched the surface of what's possible, but this should have hopefully given you plenty pointers to start experimenting.

As a next step, browse through our [example gallery](/examples) for some more realistic example use-cases, or browse the [Loggable Data Types](../reference/data_types.md) section for more simple examples of how to use the main datatypes.
