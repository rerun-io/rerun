<!--[metadata]
title = "Teleoperation over LiveKit"
steps = ["collect"]
description = "Teleoperate a remote robot over LiveKit and keep a complete recording of every episode."
source = "https://github.com/rerun-io/rerun_livekit"
tags = ["Robotics", "Live", "Streaming", "Video", "Integration"]
thumbnail = "https://static.rerun.io/teleop_livekit_thumbnail/6452e04924f63caacf2391e8e724d6e2b2719c3d/768w.png"
thumbnail_dimensions = [768, 576]
-->

This example drives a simulated quadruped over [LiveKit](https://livekit.com).
The robot logs, as normal, with the Rerun SDK.
The stream is both saved as a complete recording and forwarded over a latency-optimized bridge to the remote operator's Rerun Viewer.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/4b4fd4b4a986ae804bf877704718820cf85bd1e5_teleop_livekit.mp4" type="video/mp4" />
</video>

## Introduction

Remote teleoperation often involves poor network connections, and the operator needs to see the robot with as little latency as possible.

A recording needs every message, in order, but if the network connection is not sufficient, a stream that enforces this falls behind the robot, leading to unacceptable latency.

In this example the [`rerun_livekit`](https://github.com/rerun-io/rerun_livekit) bridge forwards a copy of the stream to the operator through LiveKit, compressing and dropping data as needed to keep the latency low.

The robot is a [Unitree Go2](https://github.com/google-deepmind/mujoco_menagerie/tree/main/unitree_go2) quadruped, simulated in [MuJoCo](https://mujoco.org/).
It walks under a pretrained policy from [MuJoCo Playground](https://playground.mujoco.org/), and you steer it from a keyboard on the operator's side.

The example covers the four parts of collecting an episode:

1. [Record](#recording-on-the-robot) on the robot, to a file and to the bridge.
2. [Stream](#streaming-to-the-operator) to the operator's Viewer, optimizing for low latency over fidelity.
3. [Drive](#driving-the-robot) with commands that go back through the same LiveKit room.
4. [Register](#both-ends-in-one-segment) both ends as two [layers](https://rerun.io/docs/catalog/layers) of one segment in a [catalog](https://rerun.io/docs/catalog/oss-server).

## Recording on the robot

The robot logs one stream to [two sinks](https://rerun.io/docs/log/sinks-and-modes/sinks#multiple-sinks-tee-pattern): an `.rrd` file with every message as it was logged, and a Rerun proxy (`rerun --serve-grpc`) that the bridge reads from.

Rerun's [MJCF importer](https://rerun.io/docs/ingest/mjcf) reads the same model file that MuJoCo simulates, so the robot in the Viewer is the robot in the simulation.

## Streaming to the operator

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/c836105b2bfc212b99c00021a12314b00715e4dc_diagram-teleop-livekit-bridge-dark.svg" alt="On the robot, a Rerun recording stream runs inside your robot code and has two sinks: a file sink that saves the recording, and a gRPC sink to a Rerun proxy that the bridge reads from. Video and data go over WebRTC through a LiveKit room to the operator's side of the bridge, which feeds the Rerun Viewer over gRPC. Commands from the operator's input device travel back through the room to the bridge on the robot, which hands them to your robot code over UDP.">
  <img class="d2-light" src="https://static.rerun.io/f3483843044fdcaf4a061f5827272125c4473cdc_diagram-teleop-livekit-bridge-light.svg" alt="On the robot, a Rerun recording stream runs inside your robot code and has two sinks: a file sink that saves the recording, and a gRPC sink to a Rerun proxy that the bridge reads from. Video and data go over WebRTC through a LiveKit room to the operator's side of the bridge, which feeds the Rerun Viewer over gRPC. Commands from the operator's input device travel back through the room to the bridge on the robot, which hands them to your robot code over UDP.">
</div>

The bridge forwards the stream through a LiveKit room, over WebRTC.
Images travel as video that adapts to the link, other data such as the lidar scans is sent on a lossy channel that skips old messages rather than waiting on them, and blueprints and static data are sent reliably.
The operator's Viewer receives the same recording, with the same entity paths and timelines.

The video at the top shows the operator's Viewer, with the latency of what arrives plotted by the bridge (`--plot-latency`).
From 14 s to 21 s in that plot the camera falls about half a second behind, while the other data keeps arriving within 20 ms, so the 3D view stays current.

## Driving the robot

The robot logs each command it receives next to the velocity it measured, so an episode holds the actions and the observations on one timeline.

<picture>
  <img src="https://static.rerun.io/teleop_livekit_velocity/ad7f08e6a44aa6ca39c5d29fbb76a67ebe431b26/full.png" alt="Two plots from the Viewer: the commanded forward velocity as a step with the measured velocity rippling around it, and the commanded turn rate as steps to the left and right with the measured turn rate following them">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/teleop_livekit_velocity/ad7f08e6a44aa6ca39c5d29fbb76a67ebe431b26/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/teleop_livekit_velocity/ad7f08e6a44aa6ca39c5d29fbb76a67ebe431b26/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/teleop_livekit_velocity/ad7f08e6a44aa6ca39c5d29fbb76a67ebe431b26/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/teleop_livekit_velocity/ad7f08e6a44aa6ca39c5d29fbb76a67ebe431b26/1200w.png">
</picture>

## Both ends in one segment

The operator's end can record too: what reached it, with lossy video and without the frames that were dropped on the way.
Both recordings keep the same recording ID, so they become two layers of one segment in a Rerun catalog.
The `robot` layer is everything the robot logged.
The `operator` layer adds the images that reached the operator and the latency of every frame.

<picture>
  <img src="https://static.rerun.io/teleop_livekit_both_ends/69188c34995a3549b23c9d9912e17231cc91242e/full.png" alt="One segment in the Viewer: the robot's sharp camera image next to the blurrier one that reached the operator, above the robot in 3D among its lidar points and plots of the latency and the forward velocity">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/teleop_livekit_both_ends/69188c34995a3549b23c9d9912e17231cc91242e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/teleop_livekit_both_ends/69188c34995a3549b23c9d9912e17231cc91242e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/teleop_livekit_both_ends/69188c34995a3549b23c9d9912e17231cc91242e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/teleop_livekit_both_ends/69188c34995a3549b23c9d9912e17231cc91242e/1200w.png">
</picture>

Opening the segment shows the robot's camera next to the operator's view on one timeline, so you can check what the operator saw at any moment of an episode.

## Run the code

This is an external example.
Check the [repository](https://github.com/rerun-io/rerun_livekit) for more information.

The repository includes the simulated robot and a keyboard driver, so everything runs on one machine.
[Try it out](https://github.com/rerun-io/rerun_livekit#try-it-out) walks through driving the robot, [Record](https://github.com/rerun-io/rerun_livekit#record) through recording on both ends, and [Both ends in one segment](https://github.com/rerun-io/rerun_livekit#both-ends-in-one-segment) through registering an episode in a local catalog.

To use the bridge with your own robot, the repository also covers:

- [Two machines](https://github.com/rerun-io/rerun_livekit#try-it-out), with LiveKit Cloud or a LiveKit server that you host.
- [Choosing what to send](https://github.com/rerun-io/rerun_livekit#choose-what-to-send), with rules for each entity.
- [Sending commands](https://github.com/rerun-io/rerun_livekit#send-commands-to-the-robot) from your own input device to your own robot.

The robot model comes from [MuJoCo Menagerie](https://github.com/google-deepmind/mujoco_menagerie) and the policy from [MuJoCo Playground](https://github.com/google-deepmind/mujoco_playground).
