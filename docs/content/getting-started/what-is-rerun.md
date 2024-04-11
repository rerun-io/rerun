---
title: What is Rerun?
order: 0
---

Rerun is an SDK and viewer for visualizing and interacting with multimodal data streams.
The SDK lets you send data from anywhere, and the viewer,
which consists of an in-memory database and a visualization engine,
collects the data and aligns it so that you can scroll back and forth in time to understand what happened.

Rerun is
- Free to use
- Simple to integrate and get started with
- Usable from C++, Python, and Rust
- Powerful, flexible, and extensible
- Built in Rust to be cross platform and fast
- Open source, dual licensed under MIT and Apache 2

Rerun is used by engineers and researchers in fields like robotics,
spatial computing, 2D/3D simulation, and finance to verify, debug, and demo.

## How do you use it?
<picture>
  <img src="https://static.rerun.io/how_to_use/fd75fa302617cd0afefc9ba6e5e1e13055fced04/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/how_to_use/fd75fa302617cd0afefc9ba6e5e1e13055fced04/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/how_to_use/fd75fa302617cd0afefc9ba6e5e1e13055fced04/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/how_to_use/fd75fa302617cd0afefc9ba6e5e1e13055fced04/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/how_to_use/fd75fa302617cd0afefc9ba6e5e1e13055fced04/1200w.png">
</picture>

1. Stream multimodal data from your code by logging it with the Rerun SDK
2. Visualize and interact with live or recorded streams, whether local or remote
3. Build layouts and customize visualizations interactively in the UI or through the SDK
4. Extend Rerun when you need to

## How does it work?
That's a big question for a welcome page. The short answer is that
Rerun goes to extreme lengths to make handling and visualizing
multimodal data streams easy and performant.

## What is Rerun for?
Rerun is built to help you understand complex processes that include rich multimodal data, including 2D, 3D, text, time series, tensors, etc.
It is used in many industries, including robotics, simulation, computer vision, or anything that involves a lot of sensors.
Let's look at a more concrete example:

### Example use case
Say you're building a vacuum cleaning robot and it keeps running into walls. Why is it doing that? You need some tool to debug it, but a normal debugger isn't gonna be helpful. Similarly, just logging text won't be very helpful either. The robot may log "Going through doorway" but that won't explain why it thinks the wall is a door.

What you need is a visual and temporal debugger, that can log all the different representations of the world the robots holds in its little head, such as:

* RGB camera feed
* depth images
* lidar scan
* segmentation image (how the robot interprets what it sees)
* its 3D map of the apartment
* all the objects the robot has detected (or thinks it has detected), as 3D shapes in the 3D map
* its confidence in its prediction
* etc

You also want to see how all these streams of data evolve over time so you can go back and pinpoint exactly what went wrong, when and why.

Maybe it turns out that a glare from the sun hit one of the sensors in the wrong way, confusing the segmentation network leading to bad object detection. Or maybe it was a bug in the lidar scanning code. Or maybe the robot thought it were somewhere else in the apartment, because its odometry was broken. Or it could be one of a thousand other things. Rerun will help you find out!

But seeing the world from the point of the view of the robot is not just for debugging - it will also give you ideas on how to improve the algorithms. It will also let you explain the brains of the robot to your colleagues, boss, and customers. And so on. Seeing is believing, and an image is worth a thousand words, and multimodal temporal logging is worth a thousand images :)

Of course, Rerun is useful for much more than just robots. Any time you have any for of sensors, or 2D or 3D state evolving over time, Rerun would be a great tool.

## Can't find what you're looking for?

- Join us in the [Rerun Community Discord](https://discord.gg/xwcxHUjD35)
- Or [submit an issue](https://github.com/rerun-io/rerun/issues) in the Rerun GitHub project

