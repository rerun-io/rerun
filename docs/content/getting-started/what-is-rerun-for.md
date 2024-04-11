---
title: What is Rerun for?
order: 1
---

Rerun is a versatile library and tool, and can be used for many things. This page is meant to give you some starting ideas on what you could use Rerun for yourself.

Say you're building a vacuum cleaning robot and it keeps running into walls. Why is it doing that? You need some tool to debug it, but a normal debugger isn't going to be helpful. Similarly, just logging text won't be very helpful either. The robot may log "Going through doorway" but that won't explain why it thinks the wall is a door.

What you need is a visual and temporal debugger, that can log all the different representations of the world the robot holds in its little head, such as:

- RGB camera feed
- depth images
- lidar scan
- Segmentation image (how the robot interprets what it sees)
- Its 3D map of the apartment
- All the objects the robot has detected (or thinks it has detected), as 3D shapes in the 3D map
- etc

You also want to see how all these streams of data evolve over time so you can go back in time and pinpoint exactly what went wrong and when.

Maybe it turns out that a glare from the sun hit one of the sensors in the wrong way, confusing the segmentation network leading to bad object detection. Or maybe it was a bug in the lidar scanning code. Or maybe the robot thought it was it was somewhere else in the apartment, because its odometry is broken. Or it could be one of a thousand other things. Rerun will help you find out!

But seeing the world from the point of view of the robot is not just for debugging - it will also give you ideas on how to improve the algorithms, new test cases to set up, or datasets to collect. It will also let you explain the brains of the robot to your colleagues, boss, and customers. And so on. Seeing is believing, and an image is worth a thousand words, and multimodal temporal logging is worth a thousand images.

Of course, Rerun is useful for much more than just robots. Any time you have any form of sensors or 2D or 3D state evolving over time, Rerun would be a great tool. And with the explosion of AI, the kind of uncertainty we're used to in robotics is becoming a widespread part of software in general.
