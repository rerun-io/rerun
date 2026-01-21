# re_mcap Architecture Notes

## Overview

The `re_mcap` crate is a sophisticated MCAP file loader that converts MCAP files into Rerun-compatible data. It uses a **layered plugin architecture** to handle different message formats.

## Directory Structure

```
crates/store/re_mcap/src/
├── layers/          # Layer implementations (plugin system)
├── parsers/         # Message decoding implementations
├── error.rs         # Error types
├── lib.rs           # Public API
└── util.rs          # Utility functions
```

## Layer System Architecture

The layer system is a **two-tier plugin architecture**:

### A. Layer Trait (File-Scoped)

**File:** `layers/mod.rs`

```rust
pub trait Layer {
    fn identifier() -> LayerIdentifier where Self: Sized;
    fn process(
        &mut self,
        mcap_bytes: &[u8],
        summary: &::mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), Error>;
}
```

- **Purpose:** Processes the entire MCAP file at once
- **Scope:** File-wide analysis (runs once)
- **Responsibility:** Extract metadata or perform global analysis

### B. MessageLayer Trait (Channel-Scoped)

**File:** `layers/mod.rs`

```rust
pub trait MessageLayer {
    fn identifier() -> LayerIdentifier where Self: Sized;
    fn init(&mut self, summary: &::mcap::Summary) -> Result<(), Error>;
    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool;
    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>>;
}
```

- **Purpose:** Handles per-channel message decoding
- **Scope:** Per-topic/channel basis
- **Key Method:** `supports_channel()` - Determines if a layer can process a channel
- **Returns:** A `MessageParser` implementation for incremental message processing

## Layer Registration & Execution

### LayerRegistry

**File:** `layers/mod.rs`

```rust
pub struct LayerRegistry {
    file_factories: BTreeMap<LayerIdentifier, fn() -> Box<dyn Layer>>,
    msg_factories: BTreeMap<LayerIdentifier, fn() -> Box<dyn MessageLayer>>,
    msg_order: Vec<LayerIdentifier>,  // Priority ordering
    fallback: Fallback,
}
```

### Built-in Layers (Priority Order)

```
LayerRegistry::all_builtin(raw_fallback_enabled)
├─ File Layers (processed first):
│  ├─ McapRecordingInfoLayer     ("recording_info")
│  ├─ McapSchemaLayer            ("schema")
│  └─ McapStatisticLayer         ("stats")
└─ Message Layers (priority order):
   ├─ McapRos2Layer              ("ros2msg")         - Semantic ROS2 handlers
   ├─ McapRos2ReflectionLayer    ("ros2_reflection") - Dynamic ROS2 reflection
   ├─ McapProtobufLayer          ("protobuf")        - Protobuf reflection
   └─ McapRawLayer               ("raw")             - Fallback (all channels)
```

### Execution Plan

```rust
pub struct ExecutionPlan {
    pub file_layers: Vec<Box<dyn Layer>>,
    pub runners: Vec<MessageLayerRunner>,
    pub assignments: Vec<LayerAssignment>,
}
```

The registry builds an execution plan that:
1. Instantiates all layers and initializes them with the MCAP summary
2. Iterates through all channels in the MCAP file
3. Uses **first-match priority** to assign each channel to a layer
4. Falls back to the global fallback layer if no layer claims a channel
5. Creates a `MessageLayerRunner` for each layer with its assigned channels

## Message Parsing Pipeline

### MessageParser Trait

**File:** `parsers/decode.rs`

```rust
pub trait MessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()>;
    fn get_log_and_publish_timepoints(
        &self,
        msg: &mcap::Message<'_>,
    ) -> anyhow::Result<Vec<re_chunk::TimePoint>>;
    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>>;
}
```

**Two-Phase Processing:**
1. **Append Phase:** Called for each message in a chunk. Parsers decode and accumulate data.
2. **Finalize Phase:** Converts all accumulated data into Rerun chunks.

### ParserContext

```rust
pub struct ParserContext {
    entity_path: EntityPath,
    pub timelines: IntMap<TimelineName, TimeColumnBuilder>,
}
```

- Manages per-topic entity paths
- Accumulates timeline information (log_time, publish_time, sensor timestamps)

## ROS2 Message Support

### Semantic Layer (McapRos2Layer)

**File:** `layers/ros2.rs`

Provides semantic handlers for specific ROS2 message types:

**Supported Messages:**
- **geometry_msgs:** `PoseStamped`
- **sensor_msgs:** `Image`, `CompressedImage`, `PointCloud2`, `CameraInfo`, `IMU`, `MagneticField`, `Range`, `BatteryState`, `JointState`, `NavSatFix`, `FluidPressure`, `Temperature`, `Illuminance`, `RelativeHumidity`
- **std_msgs:** `String`
- **tf2_msgs:** `TFMessage`
- **rcl_interfaces:** `Log`

### Reflection Layer (McapRos2ReflectionLayer)

**File:** `layers/ros2_reflection.rs`

Provides **dynamic runtime parsing** for unknown ROS2 message types:
- Parses ROS2 message schema from schema data
- Converts CDR-encoded messages into Arrow structs without semantic knowledge
- Acts as a fallback for custom ROS2 messages not in the semantic layer

## Other Layer Implementations

### File-Scoped Layers

| Layer | File | Purpose |
|-------|------|---------|
| `McapSchemaLayer` | `schema.rs` | Extracts channel/schema metadata |
| `McapStatisticLayer` | `stats.rs` | Extracts message statistics |
| `McapRecordingInfoLayer` | `recording_info.rs` | Extracts recording metadata |

### Message-Scoped Layers

| Layer | File | Purpose |
|-------|------|---------|
| `McapProtobufLayer` | `protobuf.rs` | Generic protobuf reflection (no semantic conversion) |
| `McapRawLayer` | `raw.rs` | Fallback - captures raw bytes |

## Protobuf Layer Details

**File:** `layers/protobuf.rs`

- Uses `prost_reflect` for runtime schema introspection
- Converts arbitrary protobuf messages to Arrow structs
- Handles proto3 optional fields with presence tracking
- Supports nested messages, enums, and lists
- **Does NOT produce semantic Rerun types** - only raw Arrow structs

## Data Flow

```
MCAP File
    ↓
[Read MCAP Summary & Bytes]
    ↓
LayerRegistry::plan()
    ├─ Instantiate all layers
    ├─ Call Layer::init() on each MessageLayer
    ├─ Assign channels based on supports_channel() priority
    └─ Create ExecutionPlan
    ↓
ExecutionPlan::run()
    ├─ Process all file layers
    │  └─ Layer::process() → emit Chunks
    │
    ├─ Process message layers (via MessageLayerRunner)
    │  ├─ Stream MCAP chunks
    │  ├─ For each message:
    │  │  ├─ Call MessageParser::append()
    │  │  └─ Add timepoints to ParserContext
    │  ├─ Call MessageParser::finalize()
    │  └─ Emit resulting Chunks
    │
    └─ Collector aggregates all Chunks
         ↓
    [Rerun Chunks ready for visualization]
```

## Key Traits Summary

| Name | File | Purpose |
|------|------|---------|
| `Layer` | `layers/mod.rs` | File-scoped processing |
| `MessageLayer` | `layers/mod.rs` | Channel-scoped processing |
| `MessageParser` | `parsers/decode.rs` | Per-message incremental parsing |
| `ParserContext` | `parsers/decode.rs` | Timeline and entity path management |
| `LayerRegistry` | `layers/mod.rs` | Plugin registration and planning |
| `ExecutionPlan` | `layers/mod.rs` | Concrete execution specification |
| `Ros2MessageParser` | `parsers/ros2msg/mod.rs` | ROS2-specific parser factory |

## Architectural Patterns

1. **Plugin Registration:** Factories registered at build time, instantiated lazily
2. **Priority-Based Channel Assignment:** First-match wins based on registration order
3. **Fallback Strategy:** Global fallback layer handles unassigned channels
4. **Incremental Processing:** Messages processed in chunks to manage memory
5. **Two-Phase Parsing:** Append (data accumulation) → Finalize (chunk generation)
6. **CDR Detection:** Automatic encoding format detection from message headers
7. **Runtime Reflection:** Both protobuf and ROS2 reflection for unknown types

## Gap: Foxglove Protobuf Messages

The current `McapProtobufLayer` only does generic reflection-based parsing to Arrow structs. It does **not** convert Foxglove protobuf messages (like `foxglove.CompressedImage`, `foxglove.PoseInFrame`, etc.) into semantic Rerun types.

This is the gap that the `mcap_protobuf` example addresses using the Lenses API externally. A potential integration would be to add a new `McapFoxgloveLayer` that:
1. Claims channels with Foxglove protobuf schemas
2. Uses Lenses-style transformations to convert to Rerun components
3. Sits higher priority than the generic `McapProtobufLayer`
