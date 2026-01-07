---
title: RRD format
order: 50
---

Understanding Rerun's native data format.

> **Placeholder**: This section will be expanded with detailed documentation about the RRD (Rerun Recording Data) format.

## What is RRD?

RRD is Rerun's native file format for storing recorded data. When you save data from Rerun, it's stored in `.rrd` files.

## Key characteristics

- **Streaming format**: RRD is designed for streaming data, allowing you to write and read data incrementally
- **Self-describing**: Files contain all the schema information needed to interpret the data
- **Efficient**: Optimized for fast reading and writing of time series and multimodal data
- **Portable**: Can be shared between different Rerun versions and platforms

## Working with RRD files

You can save data to RRD files using the SDK:

```python
import rerun as rr
rr.save("/path/to/recording.rrd")
```

And load them in the Viewer:

```bash
rerun /path/to/recording.rrd
```

## Related topics

- [Apps and Recordings](apps-and-recordings.md): How recordings are organized
- [Sinks](sinks.md): Different ways to output data
