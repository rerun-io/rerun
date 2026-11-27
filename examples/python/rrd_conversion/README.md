<!--[metadata]
title = "Convert to RRD format for end-to-end robotics workflows"
steps = ["collect"]
description = "One format to log, inspect, query, and train on."
tags = ["Dataset", "MCAP", "LeRobot", "Robotics"]
thumbnail = "https://static.rerun.io/rrd_conversion_thumbnail/3f5f2a2699fcf334a5b61171aced5bb75182644f/768w.png"
thumbnail_dimensions = [768, 576]
-->

Your data sources for robot learning may be diverse and not logged with Rerun: open-source datasets, historical recordings, or logs from a system that can't directly write to `.rrd`.
These come in many different file formats, yet you can still use Rerun by converting them into `.rrd`.

In `.rrd`, all sensor streams of an episode share the same timelines, so you can view and query them in a single consistent context.
You can also append new data, such as annotations, to an existing dataset and reuse the same recordings for downstream processing and training pipelines.

We've converted several open-source datasets into Rerun's [`.rrd` format](https://rerun.io/docs/log/chunks-and-rrd) so you can explore them in Rerun.

In this example, we share:

1. [Sample dataset episodes](#1-sample-dataset-episodes) that you can see on this page.
2. [Fully converted datasets](#2-fully-converted-datasets) that you can download or copy directly to your own bucket.
3. [Example code and agent skills](#3-example-code-and-agent-skills) for converting your own data into `.rrd`.

## 1. Sample dataset episodes

Below are sample episodes from each dataset, and small facts that you can notice when you inspect them in the Rerun Viewer.

### ABC-130k

[XDOF/ABC-130k](https://huggingface.co/datasets/XDOF/ABC-130k) is a large open-source bimanual teleoperation dataset: 130k episodes across about 200 household manipulation tasks, recorded as MCAP.

An episode is usually one recording of a robot working toward a task (e.g. "clean the table").
A task can be further divided into a sequence of subtasks, each covering a shorter span of time (e.g. "go to the table", "lift the cup", "wipe the table").

The ABC-130k dataset provides subtask labels, added by annotators, for a subset of its episodes.
These labels help both humans and robots understand what is going on at a given moment.
Rerun's `StateTimelineView` shows the subtask labels as colored bands along the timeline.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/50b0ffd43379c575bf01b848c89d436bfedf3cff_abc-130k-chess-00005b5f.mp4" type="video/mp4" />
</video>

Task: set_up_the_chess_pieces_on_the_board, Episode: 00005b5f ([open in the web viewer](https://rerun.io/viewer?url=https%3A%2F%2Fstatic.rerun.io%2Frrd%2F0.43.1%2Fabc-130k-chess-00005b5f_4523e539710a7fb49f7d131fbb289c59d8a5228a.rrd), 107 MB)

The colored bands make every change of subtask easy to see.

Sample 1 has only four subtask labels.
You can see that its bands mostly alternate between green (pick up) and amber (place), and the magenta `mistake` band stands out right away.
In the viewer, hover over a band to see its full label, or zoom in on the timeline to read it on the band itself.

<picture>
  <img src="https://static.rerun.io/rrd_conversion_abc_sample1_labels/8ebb82ac9452973f05b7e12260216997f9a420e7/full.png" alt="Subtask bands of ABC-130k sample 1, zoomed out, with a tooltip showing one label">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample1_labels/8ebb82ac9452973f05b7e12260216997f9a420e7/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample1_labels/8ebb82ac9452973f05b7e12260216997f9a420e7/768w.png">
</picture>

<picture>
  <img src="https://static.rerun.io/rrd_conversion_abc_sample1_labels_zoomed/01c7d35326953c07c0c5294b30f42b538d33426d/full.png" alt="Subtask bands of ABC-130k sample 1, zoomed in so the full labels show">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample1_labels_zoomed/01c7d35326953c07c0c5294b30f42b538d33426d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample1_labels_zoomed/01c7d35326953c07c0c5294b30f42b538d33426d/768w.png">
</picture>

Sample 2 is another episode of the same task ([open in the web viewer](https://rerun.io/viewer?url=https%3A%2F%2Fstatic.rerun.io%2Frrd%2F0.43.1%2Fabc-130k-chess-9e1bf3fa_8516511209038cc78dc8ea7d7a699fe7c6b0030d.rrd), 121 MB).
It has 32 different labels, because they name each chess piece and square.
You can spot this difference just by looking at the bands.

<picture>
  <img src="https://static.rerun.io/rrd_conversion_abc_sample2_labels/157a80f2629a5d515bb828e5e4f0e72472679d74/full.png" alt="Subtask bands of ABC-130k sample 2, zoomed out">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample2_labels/157a80f2629a5d515bb828e5e4f0e72472679d74/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample2_labels/157a80f2629a5d515bb828e5e4f0e72472679d74/768w.png">
</picture>

<picture>
  <img src="https://static.rerun.io/rrd_conversion_abc_sample2_labels_zoomed/8549d45620f07d567fef6a2ed0f4ab98fa37bb71/full.png" alt="Subtask bands of ABC-130k sample 2, zoomed in so the full labels show">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample2_labels_zoomed/8549d45620f07d567fef6a2ed0f4ab98fa37bb71/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rrd_conversion_abc_sample2_labels_zoomed/8549d45620f07d567fef6a2ed0f4ab98fa37bb71/768w.png">
</picture>

Checks like these help you clean up a dataset before training: you can find `mistake` segments to trim, and episodes whose labels don't match the rest.

### HIW-500

[HIW-500](https://huggingface.co/datasets/BitRobot/HIW-500) is a humanoid teleoperation dataset with 23k+ episodes of a Unitree G1 doing household tasks in real homes, recorded as MCAP.

Most sensors on a humanoid robot record local information, such as a joint angle or a wrist camera view, and many parts of the robot's body move relative to one another.
Robots can learn from these raw recordings, but humans understand humanoid motion much more easily when it's shown in 3D.

Rerun's [`rr.urdf` module](https://rerun.io/docs/ingest/urdf) and the [robot model skill](https://github.com/rerun-io/rerun/blob/main/skills/rerun-robot-model/SKILL.md) help put the different pieces together into a coherent 3D view.
Have a look at the G1 in action, and notice how much easier it is to follow the 3D motion than to track the state of every joint.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/76fc7f9330b2057ca8158a0b719c1c0b5b7ad4ac_hiw-500-sweep-floor-episode_0006.mp4" type="video/mp4" />
</video>

Task: Sweep-Floor, Episode: 2026-05-19_14-35-04/0006 ([open in the web viewer](https://rerun.io/viewer?url=https%3A%2F%2Fstatic.rerun.io%2Frrd%2F0.43.1%2Fhiw-500-sweep-floor-episode_0006_cb8e115fe3cc5552093536aadd3438c2e3a267bc.rrd), 117 MB)

### MolmoAct2

[MolmoAct2-BimanualYAM](https://huggingface.co/datasets/allenai/MolmoAct2-BimanualYAM-Dataset) is another open-source bimanual teleoperation dataset recorded with two I2RT YAM arms (the same arms as ABC-130k), stored in the LeRobot v3 format.

To view a LeRobot dataset without converting it to `.rrd`, run Rerun's built-in importer:

```sh
rerun path/to/lerobot_dataset
```

If you convert it to `.rrd` with our [LeRobotReader](https://ref.rerun.io/docs/python/stable/experimental/#rerun.experimental.LeRobotReader), you can enjoy the full capabilities of the `.rrd` format.
Below is a short sample episode with a 3D view:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/13d368050c97424f6d3ef4a4c916dc8ace736722_molmoact2-episode_12839.mp4" type="video/mp4" />
</video>

Task: Pack container, Episode: 12839 ([open in the web viewer](https://rerun.io/viewer?url=https%3A%2F%2Fstatic.rerun.io%2Frrd%2F0.43.1%2Fmolmoact2-episode_12839_7445141f6d90abeac0f85da9a33f0aad11df0617.rrd), 28 MB)

Notice how still the idle left arm stays, compared to the ABC-130k samples.
This suggests that MolmoAct2's teleoperation setup stops sending commands while the operator holds still.
This matters if you train one policy on both datasets: they use the same arms, but record idle arms differently.

### LIBERO

[LIBERO](https://libero-project.github.io/intro.html) is a popular imitation learning benchmark dataset for manipulation research.
Its original release comes in HDF5 format.

You can convert it to `.rrd` with our [Hdf5Reader](https://ref.rerun.io/docs/python/stable/experimental/#rerun.experimental.Hdf5Reader).
Below is a demo from the `libero_10` suite, teleoperated in simulation:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/d5b9bdb64763a8c64442abe5f6782c6ea830a4b1_libero-moka-pot-demo_45.mp4" type="video/mp4" />
</video>

Task: turn on the stove and put the moka pot on it, Episode: libero_10/demo_45 ([open in the web viewer](https://rerun.io/viewer?url=https%3A%2F%2Fstatic.rerun.io%2Frrd%2F0.43.1%2Flibero-moka-pot-demo_45_72224e48ccaf9e0b6246f2089c52494a2c672365.rrd), 14 MB)

The 3D view shows both cameras as frustums, so you can see where each camera image is taken from.

## 2. Fully converted datasets

The samples above only show how the datasets look in the viewer.
If you're interested in exploring more features that Rerun provides or obtaining the full datasets in `.rrd`, download them from Hugging Face at [rerun/buckets](https://huggingface.co/rerun/buckets).
Use them in your own experiments and try Rerun's catalog, query, and dataloader APIs.
Here's the list of currently available datasets:

- [ABC-130k](https://huggingface.co/buckets/rerun/abc-130k)
- [HIW-500](https://huggingface.co/buckets/rerun/hiw-500)
- [LIBERO](https://huggingface.co/buckets/rerun/libero)
- [MolmoAct2-BimanualYAM](https://huggingface.co/buckets/rerun/molmoact2)

## 3. Example code and agent skills

You can convert other datasets, including your own, into `.rrd`.

Every dataset is unique.
Re-saving your data as `.rrd` without understanding both the data and the format won't automatically make it a good dataset.

Check out our example conversion code and an agent skill that will guide your conversion.
This is the code repo that generated the converted datasets shared above.

- [rrd-datasets](https://github.com/rerun-io/rrd-datasets) — An open-source project with runnable conversion workflows and an agent skill.

Rerun also provides agent skills and readers that you can use for the conversion:

- [Rerun skills](https://github.com/rerun-io/rerun/tree/main/skills) — A collection of skills that help AI agents work with Rerun and multimodal data.
- Readers for common formats: `Hdf5Reader`, `LeRobotReader`, `McapReader`, `Mp4Reader`, and `ParquetReader`.
