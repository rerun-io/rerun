---
title: Send from C++
order: 1
---

In this section we'll log and visualize our first non-trivial dataset, putting many of Rerun's core concepts and features to use.

In a few lines of code, we'll go from a blank sheet to something you don't see every day: an animated, interactive, DNA-shaped abacus:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/c4c4ef1e4a1b25002da7c44d4316b0e07ae8d6ed_logging_data1_result.webm" type="video/webm" />
</video>

This guide aims to go wide instead of deep.
There are links to other doc pages where you can learn more about specific topics.

At any time, you can checkout the complete code listing for this tutorial [here](https://github.com/rerun-io/rerun/tree/latest/examples/cpp/dna/main.cpp) to better keep track of the overall picture.
To build the example from the repository, run:

```bash
cd examples/cpp/dna
cmake -B build
cmake --build build -j
```

And then to run it on Linux/Mac:

```
./build/example_dna
```

and Windows respectively:

```
build\Debug\example_dna.exe
```

## Prerequisites

You should have already [installed the viewer](../../overview/installing-rerun.md).

We assume you have a working C++ toolchain and are using `CMake` to build your project. For this example
we will let Rerun download build [Apache Arrow](https://arrow.apache.org/)'s C++ library itself.
To learn more about how Rerun's CMake script can be configured, see [CMake Setup in Detail](https://ref.rerun.io/docs/cpp/stable/md__2home_2runner_2work_2rerun_2rerun_2rerun__cpp_2cmake__setup__in__detail.html) in the C++ reference documentation.

## Setting up your CMakeLists.txt

A minimal CMakeLists.txt for this example looks like this:

```cmake
cmake_minimum_required(VERSION 3.16...3.27)
project(example_dna LANGUAGES CXX)

add_executable(example_dna main.cpp)

# Download the rerun_sdk
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)

# Link against rerun_sdk.
target_link_libraries(example_dna PRIVATE rerun_sdk)
```

Note that Rerun requires at least C++17. Depending on the sdk will automatically ensure that C++17 or newer is enabled.

## Includes

To use Rerun all you need to include is `rerun.hpp`, however for this example we will pull in a few extra headers.

Starting our `main.cpp`:

```cpp
#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

#include <algorithm> // std::generate
#include <random>
#include <vector>

using namespace rerun::demo;
using namespace std::chrono_literals;

static constexpr size_t NUM_POINTS = 100;
```

## Initializing the SDK

To get going we want to create a [`RecordingStream`](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/src/rerun/recording_stream.hpp), which is the main interface for sending data to Rerun.
When creating the `RecordingStream` we also need to specify the name of the application we're working on
by setting it's `ApplicationId`.

We then use the stream to spawn a new Rerun Viewer via [`spawn`](https://github.com/rerun-io/rerun/blob/d962b34b07775bbacf14883d683cca6746852b6a/rerun_cpp/src/rerun/recording_stream.hpp#L151).

Add our initial `main` to `main.cpp`:

```cpp
int main() {
    auto rec = rerun::RecordingStream("rerun_example_dna_abacus");
    rec.spawn().exit_on_failure();
}
```

Among other things, a stable `ApplicationId` will make it so the [Rerun Viewer](../../reference/viewer/overview.md) retains its UI state across runs for this specific dataset, which will make our lives much easier as we iterate.

Check out the reference to learn more about how Rerun deals with [recordings and datasets](../../concepts/logging-and-ingestion/recordings-and-datasets.md).

## Testing our app

Even though we haven't logged any data yet this is a good time to verify everything is working.

```bash
cmake -B build
cmake --build build -j
./build/example_dna
```

When everything finishes compiling, an empty Rerun Viewer should be spawned:

<picture>
  <img src="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun-welcome-screen-0.9/cc45a0700ccf02016fb942153106db4af0c224db/1200w.png">
</picture>

## Logging our first points

Now let's add some data to the viewer.

The core structure of our DNA looking shape can easily be described using two point clouds shaped like spirals.
Add the following to your `main` function:

```cpp
std::vector<rerun::Position3D> points1, points2;
std::vector<rerun::Color> colors1, colors2;
color_spiral(NUM_POINTS, 2.0f, 0.02f, 0.0f, 0.1f, points1, colors1);
color_spiral(NUM_POINTS, 2.0f, 0.02f, TAU * 0.5f, 0.1f, points2, colors2);

rec.log(
    "dna/structure/left",
    rerun::Points3D(points1).with_colors(colors1).with_radii({0.08f})
);
rec.log(
    "dna/structure/right",
    rerun::Points3D(points2).with_colors(colors2).with_radii({0.08f})
);
```

Re-compile and run your program again:

```bash
cmake --build build -j
./build/example_dna
```

and now you should now see this scene in the viewer:

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

The easiest way to log geometric primitives is the use the [`RecordingStream::log`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a7badac918d44d66e04e948f38818ff11) method with one of the built-in archetype class, such as [`Points3D`](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/src/rerun/archetypes/points3d.hpp). Archetypes take care of building batches of components that are recognized and correctly displayed by the Rerun viewer.

### Components

Under the hood, the Rerun C++ SDK logs individual _components_ like positions, colors,
and radii. Archetypes are just one high-level, convenient way of building such collections of components. For advanced use
cases, it's possible to add custom components to archetypes, or even log entirely custom sets of components, bypassing
archetypes altogether.
For more information on how the Rerun data model works, refer to our section on [Entities and Components](../../concepts/logging-and-ingestion/entity-component.md).

Notably, the [`RecordingStream::log`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a7badac918d44d66e04e948f38818ff11) method
will handle any data type that implements the [`AsComponents<T>`](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/src/rerun/as_components.hpp) trait, making it easy to add your own data.
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

```cpp
std::vector<rerun::LineStrip3D> lines;
for (size_t i = 0; i < points1.size(); ++i) {
    lines.emplace_back(rerun::LineStrip3D({points1[i].xyz, points2[i].xyz}));
}

rec.log(
    "dna/structure/scaffolding",
    rerun::LineStrips3D(lines).with_colors(rerun::Color(128, 128, 128))
);
```

Which only leaves the beads:

```cpp
std::default_random_engine gen;
std::uniform_real_distribution<float> dist(0.0f, 1.0f);
std::vector<float> offsets(NUM_POINTS);
std::generate(offsets.begin(), offsets.end(), [&] { return dist(gen); });

std::vector<rerun::Position3D> beads_positions(lines.size());
std::vector<rerun::Color> beads_colors(lines.size());

for (size_t i = 0; i < lines.size(); ++i) {
    float offset = offsets[i];
    auto c = static_cast<uint8_t>(bounce_lerp(80.0f, 230.0f, offset * 2.0f));

    beads_positions[i] = rerun::Position3D(
        bounce_lerp(lines[i].points[0].x(), lines[i].points[1].x(), offset),
        bounce_lerp(lines[i].points[0].y(), lines[i].points[1].y(), offset),
        bounce_lerp(lines[i].points[0].z(), lines[i].points[1].z(), offset)
    );
    beads_colors[i] = rerun::Color(c, c, c);
}

rec.log(
    "dna/structure/scaffolding/beads",
    rerun::Points3D(beads_positions).with_colors(beads_colors).with_radii({0.06f})
);
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

Let's add our custom timeline.

Replace the section that logs the beads with a loop that logs the beads at different timestamps:

```cpp
for (int t = 0; t < 400; t++) {
    auto time = std::chrono::duration<float>(t) * 0.01f;

    rec.set_time_duration("stable_time", time);

    for (size_t i = 0; i < lines.size(); ++i) {
        float time_offset = time.count() + offsets[i];
        auto c = static_cast<uint8_t>(bounce_lerp(80.0f, 230.0f, time_offset * 2.0f));

        beads_positions[i] = rerun::Position3D(
            bounce_lerp(lines[i].points[0].x(), lines[i].points[1].x(), time_offset),
            bounce_lerp(lines[i].points[0].y(), lines[i].points[1].y(), time_offset),
            bounce_lerp(lines[i].points[0].z(), lines[i].points[1].z(), time_offset)
        );
        beads_colors[i] = rerun::Color(c, c, c);
    }

    rec.log(
        "dna/structure/scaffolding/beads",
        rerun::Points3D(beads_positions).with_colors(beads_colors).with_radii({0.06f})
    );
}
```

First we use [`RecordingStream::set_time_secs`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#ad735156502aea8eecd0a5eb2f6678d55) to declare our own custom `Timeline` and set the current timestamp.
You can add as many timelines and timestamps as you want when logging data.

⚠️ If you run this code as is, the result will be.. surprising: the beads are animating as expected, but everything we've logged until that point is gone! ⚠️

![logging data - wat](https://static.rerun.io/a396c8aae1cbd717a3f35472594f789e4829b1ae_logging_data7_wat.png)

Enter…

### Latest-at semantics

That's because the Rerun Viewer has switched to displaying your custom timeline by default, but the original data was only logged to the _default_ timeline (called `log_time`).
To fix this, go back to the top of your main and initialize your timeline before logging the initial structure:

```cpp
rec.set_time_duration_secs("stable_time", 0.0f);

rec.log(
    "dna/structure/left",
    rerun::Points3D(points1).with_colors(colors1).with_radii({0.08f})
);
rec.log(
    "dna/structure/right",
    rerun::Points3D(points2).with_colors(colors2).with_radii({0.08f})
);
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

```cpp
for (int t = 0; t < 400; t++) {
    auto time = std::chrono::duration<float>(t) * 0.01f;

    rec.set_time_duration("stable_time", time);

    rec.log(
        "dna/structure",
        rerun::archetypes::Transform3D(rerun::RotationAxisAngle(
            {0.0f, 0.0f, 1.0f},
            rerun::Angle::radians(time.count() / 4.0f * TAU)
        ))
    );
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

-   Use [`RecordingStream::save`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a555a7940a076c93d951de5b139d14918) to stream all logging data to disk.
-   Visualize it via `rerun path/to/recording.rrd`

You can also save a recording (or a portion of it) as you're visualizing it, directly from the viewer.

### RRD file backwards compatibility

RRD files saved with Rerun 0.23 or later can be opened with a newer Rerun version.
For more details and potential limitations, please refer to [our blog post](https://rerun.io/blog/release-0.23).

⚠️ At the moment, we only guarantee compatibility across adjacent minor versions (e.g. Rerun 0.24 can open RRDs from 0.23).

### Closing

This closes our whirlwind tour of logging with Rerun. We've barely scratched the surface of what's possible, but this should have hopefully given you plenty pointers to start experimenting.

As a next step, browse through our [example gallery](/examples) for some more realistic example use-cases, browse the [Types](../../reference/types.md) section for more simple examples of how to use the main data types, or dig deeper into [querying your logged data](../data-out.md).
