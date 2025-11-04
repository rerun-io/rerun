---
title: What is Rerun for?
order: 100
---

Rerun is built to help you understand and improve complex processes that include rich multimodal data, like 2D, 3D, text, time series, tensors, etc.
It is used in many industries, including robotics, simulation, computer vision,
or anything that involves a lot of sensors or other signals that evolve over time.

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

Of course, Rerun is useful for much more than just robots. Any time you have any form of sensors, or 2D or 3D state evolving over time, Rerun is a great tool.
