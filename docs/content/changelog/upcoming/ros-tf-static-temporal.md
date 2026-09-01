---
title: "MCAP importer preserves ROS `/tf_static` message timing"
hidden: true
type: breaking
---

### MCAP importer preserves ROS `/tf_static` message timing

The MCAP importer no longer maps the ROS `/tf_static` topic to Rerun static data.
It now preserves the original MCAP message timestamps.

This does not impact schema stability for registration on an existing catalog dataset.
However, if you used queries that previously used the static-only index, you may need to adapt them to use an index:

```py
# Before
df = dataset.reader(index=None)

# After
df = dataset.reader(index="message_log_time")
```

Rationale for this change:

- ROS' TF buffer requires the static-transform semantic because it otherwise works on a rolling time window.
  In contrast, Rerun doesn't require this concept because it doesn't use a time window and can retrieve transforms through a latest-at query from the first time they appear onward.
- `message_log_time` and `message_publish_time` can show when a transform appeared and help diagnose an incomplete transform tree.
- Foxglove frame transforms on `/tf_static` already behave this way.
