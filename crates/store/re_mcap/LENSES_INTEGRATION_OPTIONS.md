# Lenses Integration Options for re_mcap

## Goal

Add support for Foxglove Protobuf messages in the builtin MCAP dataloader by using the Lenses API to transform the Arrow output from `McapProtobufLayer` into semantic Rerun components.

## Current Architecture

```
MCAP File
    ↓
LayerRegistry::plan()
    ↓
ExecutionPlan::run()
    ├─ File layers (metadata extraction)
    └─ Message layers (first-match priority):
       ├─ McapRos2Layer         → Semantic Rerun components
       ├─ McapRos2ReflectionLayer → Arrow structs
       ├─ McapProtobufLayer     → Arrow structs (no semantic conversion)
       └─ McapRawLayer          → Raw bytes (fallback)
    ↓
emit(Chunk) callback
    ↓
Rerun viewer/storage
```

## The Challenge

The current layer system has **no post-processing concept**. Each `MessageLayer`:
1. Claims channels via `supports_channel()`
2. Produces chunks via `MessageParser::finalize()`
3. Chunks are emitted directly to the callback

There's no mechanism for one layer to transform another layer's output.

## Integration Options

### Option A: Post-Processing Step in ExecutionPlan

**Concept:** Add a post-processing phase after all layers produce chunks.

```rust
pub struct ExecutionPlan {
    pub file_layers: Vec<Box<dyn Layer>>,
    pub runners: Vec<MessageLayerRunner>,
    pub assignments: Vec<LayerAssignment>,
    pub post_processors: Vec<Lenses>,  // NEW
}

impl ExecutionPlan {
    pub fn run(..., emit: &mut dyn FnMut(Chunk)) {
        // ... existing layer processing ...

        // NEW: Wrap emit to apply Lenses
        let mut emit_with_lenses = |chunk: Chunk| {
            for lens_result in self.apply_lenses(&chunk) {
                emit(lens_result);
            }
        };

        for runner in &mut self.runners {
            runner.process(mcap_bytes, summary, &mut emit_with_lenses)?;
        }
    }
}
```

**Configuration:**
```rust
let registry = LayerRegistry::all_builtin(true)
    .with_lenses(foxglove_lenses());  // NEW method
```

**Pros:**
- Clean separation of concerns (decoding vs transformation)
- Reuses existing `McapProtobufLayer` without modification
- Similar pattern to the `mcap_protobuf` example
- Lenses can be configured independently of layers

**Cons:**
- New field in `ExecutionPlan`
- Need to decide how to handle `OutputMode` (forward unmatched, etc.)
- Lenses applied to ALL chunks, not just protobuf (need filtering)

**Similarity to mcap_protobuf example:** HIGH - same Lenses, just different integration point.

---

### Option B: New McapFoxgloveLensesLayer (Higher Priority)

**Concept:** A dedicated layer that claims Foxglove channels and handles them end-to-end.

```rust
pub struct McapFoxgloveLensesLayer {
    protobuf_layer: McapProtobufLayer,  // Reuse for decoding
    lenses: Lenses,
}

impl MessageLayer for McapFoxgloveLensesLayer {
    fn identifier() -> LayerIdentifier { "foxglove_lenses".into() }

    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool {
        // Only claim Foxglove protobuf channels
        channel.schema.as_ref()
            .map(|s| s.encoding == "protobuf" && s.name.starts_with("foxglove."))
            .unwrap_or(false)
    }

    fn message_parser(&self, channel: &mcap::Channel<'_>, num_rows: usize)
        -> Option<Box<dyn MessageParser>>
    {
        // Create parser that decodes protobuf + applies lenses
        Some(Box::new(FoxgloveLensesParser::new(channel, num_rows, &self.lenses)))
    }
}
```

**Registration:**
```rust
LayerRegistry::all_builtin(true)
    .register_message_layer::<McapFoxgloveLensesLayer>()  // Before McapProtobufLayer
    .register_message_layer::<McapRos2Layer>()
    // ...
```

**Pros:**
- Follows existing layer patterns exactly
- Self-contained - no changes to `ExecutionPlan`
- Clear priority: Foxglove layer claims channels before generic protobuf

**Cons:**
- Needs custom `MessageParser` that combines protobuf decoding + lenses
- Some code duplication with `McapProtobufLayer` (or composition)
- Lenses applied during parsing, not as post-processing

**Similarity to mcap_protobuf example:** MEDIUM - same Lenses, but different application point.

---

### Option C: Wrap the emit Callback

**Concept:** The simplest approach - wrap the emit callback to apply Lenses.

```rust
impl ExecutionPlan {
    pub fn run_with_lenses(
        self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        lenses: &Lenses,
        emit: &mut dyn FnMut(Chunk),
    ) -> anyhow::Result<()> {
        let mut emit_wrapped = |chunk: Chunk| {
            for result in lenses.apply(&chunk) {
                match result {
                    Ok(new_chunk) => emit(new_chunk),
                    Err(partial) => { /* handle */ }
                }
            }
        };
        self.run(mcap_bytes, summary, &mut emit_wrapped)
    }
}
```

**Pros:**
- Minimal code changes
- No new traits or structs
- Easy to understand

**Cons:**
- Less discoverable API
- Caller must manage Lenses separately
- No integration with `LayerRegistry` configuration

**Similarity to mcap_protobuf example:** HIGH - essentially the same pattern.

---

### Option D: LensesSink at Dataloader Level

**Concept:** Keep `re_mcap` unchanged; integrate Lenses at the dataloader level.

This is essentially what the `mcap_protobuf` example does:

```rust
// In the MCAP dataloader (outside re_mcap)
let lenses_sink = LensesSink::new(underlying_sink)
    .with_lens(foxglove_image_lens())
    .with_lens(foxglove_pose_lens())
    // ...
```

**Pros:**
- Already proven pattern (mcap_protobuf example works)
- No changes to `re_mcap` crate
- Maximum flexibility for users

**Cons:**
- Not "builtin" - requires dataloader modification
- Lenses applied after chunks are serialized/deserialized (less efficient)
- Configuration separate from layer system

**Similarity to mcap_protobuf example:** IDENTICAL - this IS the mcap_protobuf approach.

---

### Option E: Configurable Lenses in McapProtobufLayer

**Concept:** Add optional Lenses configuration to the existing protobuf layer.

```rust
pub struct McapProtobufLayer {
    schemas: HashMap<String, DynamicMessageDescriptor>,
    lenses: Option<Lenses>,  // NEW
}

impl McapProtobufLayer {
    pub fn with_lenses(mut self, lenses: Lenses) -> Self {
        self.lenses = Some(lenses);
        self
    }
}
```

The parser would apply lenses during `finalize()`:

```rust
fn finalize(self, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
    let chunks = self.build_chunks(ctx)?;

    if let Some(lenses) = &self.lenses {
        chunks.into_iter()
            .flat_map(|c| lenses.apply(&c))
            .collect()
    } else {
        Ok(chunks)
    }
}
```

**Pros:**
- Single layer handles both decoding and transformation
- No new layers or traits needed
- Natural place for protobuf-specific transforms

**Cons:**
- Couples generic protobuf with Foxglove-specific logic
- Configuration less flexible
- Layer becomes more complex

**Similarity to mcap_protobuf example:** MEDIUM - same Lenses, integrated differently.

---

## Recommendation

**For maximum similarity to mcap_protobuf example: Option A or Option C**

These options apply Lenses as a post-processing step on already-decoded Arrow data, which is exactly what the mcap_protobuf example does via `LensesSink`.

**Option A (Post-Processing in ExecutionPlan)** is the cleanest integration:

1. Keeps layer system unchanged
2. Lenses configured via `LayerRegistry`
3. Same Lens definitions work as in mcap_protobuf example
4. Clear separation: `McapProtobufLayer` decodes, Lenses transform

**Example usage would look like:**

```rust
// Define Foxglove lenses (same as mcap_protobuf example)
fn foxglove_lenses() -> Lenses {
    Lenses::new()
        .with_lens(foxglove_image_lens())
        .with_lens(foxglove_pose_lens())
        .with_lens(foxglove_transforms_lens())
        // ...
}

// Configure registry
let registry = LayerRegistry::all_builtin(true)
    .with_post_processor(foxglove_lenses());

// Use as before
let plan = registry.plan(&summary)?;
plan.run(mcap_bytes, &summary, &mut emit)?;
```

**The Lens definitions themselves would be nearly identical to the mcap_protobuf example**, just moved into a module within `re_mcap` or a shared crate.

---

## Data Flow Comparison

### Current (mcap_protobuf example)
```
MCAP → McapProtobufLayer → Chunk → LogMsg → LensesSink → Transformed Chunk → Viewer
                                              ↑
                                    (decode → apply lenses → encode)
```

### Proposed (Option A)
```
MCAP → McapProtobufLayer → Chunk → Lenses Post-Processor → Transformed Chunk → emit
                                              ↑
                                    (apply lenses directly on Chunk)
```

The key difference: Option A avoids the extra encode/decode cycle since Lenses are applied directly to `Chunk` objects before they leave `re_mcap`.

---

## Implementation Considerations

1. **Lens Input Column Names**: The mcap_protobuf example uses `"foxglove.PoseInFrame:message"` as the input column name. This must match what `McapProtobufLayer` produces.

2. **Schema Detection**: Need to verify `McapProtobufLayer` output column names include the schema name (e.g., `"foxglove.CompressedImage:message"`).

3. **OutputMode**: Decide whether to forward unmatched chunks, drop them, or forward all. The mcap_protobuf example uses `ForwardUnmatched`.

4. **Error Handling**: Lenses can produce `PartialChunk` on errors. Need to decide how to handle these in the layer context.

5. **Timeline Extraction**: Lenses can extract timelines from component data (e.g., Foxglove `timestamp` field). This is important for proper time synchronization.
