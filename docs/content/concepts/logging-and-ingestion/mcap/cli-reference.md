---
title: CLI Reference for MCAP
order: 500
---

This reference guide covers all command-line options and workflows for working with MCAP files in Rerun.

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

## Layer selection

### Using specific layers

Control which processing layers are applied during conversion:

```bash
# Use only protobuf decoding and file statistics
rerun mcap convert input.mcap -l protobuf -l stats -o output.rrd

# Use only ROS2 semantic interpretation for robotics data
rerun mcap convert input.mcap -l ros2msg -o output.rrd

# Combine multiple layers for comprehensive data access
rerun mcap convert input.mcap -l ros2msg -l raw -l recording_info -o output.rrd
```

### Available layer options

- **`raw`**: Preserve original message bytes
- **`schema`**: Extract metadata and schema information
- **`stats`**: Compute file and channel statistics
- **`protobuf`**: Decode protobuf messages using into generic Arrow data without Rerun visualization components
- **`ros2msg`**: Semantic interpretation of ROS2 messages
- **`recording_info`**: Extract recording session metadata

### Default behavior

When no `-l` flags are specified, all available layers are used:

```bash
# These commands are equivalent (default uses all layers):

rerun mcap convert input.mcap -o output.rrd

rerun mcap convert input.mcap \
    -l raw \
    -l schema \
    -l stats \
    -l protobuf \
    -l ros2msg \
    -l recording_info \
    -o output.rrd
```
