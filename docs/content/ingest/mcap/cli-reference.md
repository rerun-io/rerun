---
title: CLI Reference for MCAP
order: 500
description: Command-line options for viewing, converting, and decoding MCAP files
---

This guide covers the common command-line workflows for MCAP files in Rerun.
For every option, see the [`rerun mcap` reference](../../reference/cli.md#rerun-mcap).

## Basic commands

### Direct viewing

Open MCAP files directly in the Rerun Viewer:

```bash
# View a single MCAP file
rerun data.mcap

# View multiple specific files
rerun file1.mcap file2.mcap file3.mcap

# Use glob patterns to load all MCAP files in a directory
rerun recordings/*.mcap

# Recursively load all MCAP files from a directory
rerun mcap_data/
```

### File conversion

Convert MCAP files to Rerun's native RRD format:

```bash
# Convert MCAP to RRD format for faster loading
rerun mcap convert input.mcap -o output.rrd

# Convert with custom output location
rerun mcap convert data.mcap -o /path/to/output.rrd
```

## Decoder selection

### Using specific decoders

Control which processing decoders are applied during conversion:

```bash
# Use only protobuf decoding and file statistics
rerun mcap convert input.mcap -d protobuf -d stats -o output.rrd

# Use only ROS2 semantic interpretation for robotics data
rerun mcap convert input.mcap -d ros2msg -o output.rrd

# Add robot geometry from ROS robot_description topics
rerun mcap convert input.mcap -d ros2msg -d urdf -o output.rrd

# Combine multiple decoders for comprehensive data access
rerun mcap convert input.mcap -d ros2msg -d raw -d recording_info -o output.rrd
```

For what each decoder does and which ones run by default, see [MCAP decoders explained](decoders-explained.md).
For every option of `rerun mcap convert`, see the [CLI reference](../../reference/cli.md#rerun-mcap-convert).
