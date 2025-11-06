# Blueprint Generation Cache Invalidation Issue

## Investigation Summary

This document describes the blueprint generation issue identified in PR #11743 review by @Wumpf.

## Root Cause

In `crates/viewer/re_viewer_context/src/time_control.rs:48-54`, during playback the time cursor is continuously written to the blueprint:

```rust
fn set_time(&self, time: impl Into<TimeInt>) {
    self.save_static_blueprint_component(
        time_panel_blueprint_entity_path(),
        &TimePanelBlueprint::descriptor_time(),
        &re_types::blueprint::components::TimeInt(time.as_i64().into()),
    );
}
```

Every time this writes to the blueprint store, it increments the `blueprint_generation`. Since both cache keys in `app_state.rs` include `blueprint_generation`:

```rust
// Line 185-189 in app_state.rs
struct VisualizableEntitiesCacheKey {
    recording_generation: re_chunk_store::ChunkStoreGeneration,
    blueprint_generation: re_chunk_store::ChunkStoreGeneration,
    space_origin: EntityPath,
}

// Line 159-163 in app_state.rs
struct ViewQueryCacheKey {
    recording_generation: re_chunk_store::ChunkStoreGeneration,
    blueprint_generation: re_chunk_store::ChunkStoreGeneration,
    blueprint_query: LatestAtQuery,
}
```

**The cache is invalidated every frame during playback**, making it only effective when the viewer is paused.

## Impact

As @Wumpf noted in the review:
> "The blueprint generation now changes continuously when playing due to the time cursor being stored there, making this cache a lot less effective and really only useful for a standstill viewer."

This significantly reduces the performance benefits of the caching optimization.

## Proposed Solutions

### Option 1: Remove blueprint_generation from VisualizableEntitiesCacheKey ⭐ RECOMMENDED

**Complexity:** Simple

**Rationale:**
- Visualizable entities only depend on:
  - Recording data (which entities exist and what components they have)
  - View configuration (space_origin, view class)
  - **NOT** on the blueprint time cursor
- We can safely remove `blueprint_generation` from `VisualizableEntitiesCacheKey`
- Keep `recording_generation` since new entities being added should invalidate the cache
- This would make the visualizable entities cache effective during playback

**Changes required:**
```rust
// app_state.rs:185-189
struct VisualizableEntitiesCacheKey {
    recording_generation: re_chunk_store::ChunkStoreGeneration,
    // Remove: blueprint_generation: re_chunk_store::ChunkStoreGeneration,
    space_origin: EntityPath,
}
```

### Option 2: Make blueprint_query comparison sufficient for ViewQueryCacheKey

**Complexity:** Moderate

**Rationale:**
- The `ViewQueryCacheKey` already includes `blueprint_query` which captures the actual time
- We could remove `blueprint_generation` from `ViewQueryCacheKey` and rely solely on the query comparison
- However, this might miss other blueprint changes (view contents, property overrides)

**Concerns:**
- Query results may depend on blueprint property overrides that could change
- Need to verify that `blueprint_query` comparison catches all relevant changes

### Option 3: Split blueprint generation into "structural" vs "state" changes

**Complexity:** High

**Rationale:**
- Add a separate generation counter for structural blueprint changes (views, containers, space origins)
- Keep time cursor changes separate from structural changes
- This would require changes to the chunk store architecture

**Changes required:**
- Modify `re_chunk_store` to support multiple generation counters
- Categorize blueprint writes into "structural" vs "state"
- Update all cache keys to use the appropriate generation counter

### Option 4: Track specific blueprint entities that affect each cache

**Complexity:** Very High

**Rationale:**
- Instead of using global generation, track which specific blueprint entity paths affect each cache
- More precise invalidation
- Significantly more complex to implement and maintain

**Changes required:**
- Design a dependency tracking system
- Update all caches to register their dependencies
- Monitor blueprint changes and invalidate only affected caches

## Recommendation

**Start with Option 1** for `VisualizableEntitiesCacheKey`. This is the safest and simplest fix:
- ✅ Visualizable entities truly don't depend on blueprint generation
- ✅ The recording generation is sufficient to invalidate when new entities arrive
- ✅ This alone would restore cache effectiveness during playback for the most expensive operation
- ✅ Low risk of introducing bugs
- ✅ Easy to implement and test

For `ViewQueryCacheKey`, we need to be more careful since query results might depend on blueprint property overrides that could change. We could:
- Keep it as-is for now, OR
- Make it smarter by only including the blueprint_generation when we detect structural changes

## Next Steps

1. Implement Option 1 to fix `VisualizableEntitiesCacheKey`
2. Test with playback to verify cache effectiveness
3. Profile to measure performance improvement
4. Evaluate whether `ViewQueryCacheKey` needs similar treatment based on profiling results
5. Consider Option 3 as a longer-term architectural improvement if needed
