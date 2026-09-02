---
title: "ROS 2 MCAP importer outputs `VideoStream:codec` as temporal data"
hidden: true
type: breaking
---

### ROS 2 MCAP importer outputs `VideoStream:codec` as temporal data

When importing a ROS 2 `sensor_msgs/msg/CompressedImage` topic with the `h264` format, the `VideoStream:codec` component is now present at each message timestamp instead of as static data.

This does not impact schema stability for registration on an existing catalog dataset.
However, if you used queries that previously used the static-only index, you may need to adapt them to use an index:

```py
# Before
df = dataset.reader(index=None)

# After
df = dataset.reader(index="message_log_time")
```
