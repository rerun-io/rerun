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

### Available decoder options

Decoding:
- **`raw`**: Preserve original message bytes
- **`schema`**: Extract metadata and schema information
- **`stats`**: Compute file and channel statistics
- **`metadata`**: Extract metadata records into RRD `__properties`, if present
- **`protobuf`**: Decode protobuf messages using into generic Arrow data without Rerun visualization components
- **`recording_info`**: Extract recording session metadata
- **`urdf`**: Use Rerun's built-in URDF loader when a ROS 2 `/robot_description` topic is present

Semantic:
- **`foxglove`**: Semantic interpretation of Foxglove Protobuf messages
- **`ros2msg`**: Semantic interpretation of ROS2 messages

### Default behavior

When no `-d` flags are specified, all available decoders are used:

```bash
# These commands are equivalent (default uses all decoders):

rerun mcap convert input.mcap -o output.rrd

rerun mcap convert input.mcap \
    -d raw \
    -d schema \
    -d stats \
    -d metadata \
    -d protobuf \
    -d recording_info \
    -d urdf \
    -d ros2msg \
    -d foxglove \
    -o output.rrd
```
