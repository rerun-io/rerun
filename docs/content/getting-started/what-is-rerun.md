---
title: What is Rerun?
order: 0
---

Rerun is building the multimodal data stack to model, ingest, store, query and view robotics-style data.
It's built to help you understand and improve complex processes that include rich multimodal data, like 2D, 3D, text, time series, tensors, etc.
It is used in many industries, including robotics, spatial and embodied AI, generative media, industrial processing, simulation, security, and health.

## Open source: visualization and log handling
The open source project combines simple and flexible log handling with a fast, embeddable visualizer.
It’s easy to get started and can be used as a stand alone library.

The data model is a time aware Entity Component System (ECS), designed for domains like robotics and XR.
The project includes a custom database query engine and rendering engine, both built around this model.

## Commercial: multimodal data handling at scale
The commercial offering is managed infrastructure to ingest, store, analyze, and stream large amounts of robotics-style data.
It's built around Rerun's open source data model to make data pipelines simple to build, and easy to operate with built-in visual debugging.

It gives you a single database interface to operate on data from multiple sources, including MCAP, proprietary log-formats, LeRobot Datasets,
and multimodal table formats like Lance.

It's under development with select partners. [Get in touch](https://5li7zhj98k8.typeform.com/to/a5XDpBkZ) if you'd like to be one of them.


## How do you use it?

<picture>
  <img src="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/1200w.png">
</picture>

1. Use the Rerun SDK to log multimodal data from your code or load it from storage
2. View live or recorded data in the standalone viewer or embedded in your app
3. Build layouts and customize visualizations interactively in the UI or through the SDK
4. Query recordings to get clean dataframes into tools like Pandas, Polars, or DuckDB
5. Extend Rerun when you need to

## What is Rerun for?

Rerun is particularly valuable any time you have sensors, or 2D or 3D state evolving over time, where normal debugging tools fall short.

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

Maybe it turns out that a glare from the sun hit one of the sensors in the wrong way, confusing the segmentation network leading to bad object detection. Or maybe it was a bug in the lidar scanning code. Or maybe the robot thought it was somewhere else in the apartment, because its odometry was broken. Or it could be one of a thousand other things. Rerun will help you find out!

But seeing the world from the point of the view of the robot is not just for debugging - it will also give you ideas on how to improve the algorithms, new test cases to set up, or datasets to collect. It will also let you explain the brains of the robot to your colleagues, boss, and customers. And so on. Seeing is believing, and an image is worth a thousand words, and multimodal temporal logging is worth a thousand images :)

While seeing and understanding your data is core to making progress in robotics, there is one more thing:
You can also use the data you collected for visualization to create new datasets for training and evaluating the models and algorithms that run on your robot.
Rerun provides query APIs to make it easy to extract clean datasets from your recording for exactly that purpose.

## How does it work?
That's a big question for a welcome page. The short answer is that
Rerun goes to extreme lengths to make handling and visualizing
multimodal data streams easy and performant.

## Can't find what you're looking for?

- Join us in the [Rerun Community Discord](https://discord.gg/xwcxHUjD35)
- Or [submit an issue](https://github.com/rerun-io/rerun/issues) in the Rerun GitHub project

