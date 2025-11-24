# Boxes3D Fast-Path Snapshot Mismatch (instanced box cloud)

## Objective
Make the instanced box cloud renderer produce a pixel-identical image to the legacy procedural path without updating the snapshot. Current snapshot test (`crates/viewer/re_view_spatial/tests/boxes3d_fast_path.rs`) fails only on the cube; the background/grid match.

## Current code state (relevant to the mismatch)
- **Renderer:** `crates/viewer/re_renderer/src/renderer/box_cloud.rs`
  - Instanced rendering: unit cube vertex buffer + instance buffer.
  - Vertex format: position/normal (float3) + per-instance attributes: center (float3), half_size_x (float), half_size_yz (float2), color (`Unorm8x4`), picking (`Uint32x2`).
  - Default batch flags: shading disabled (no lighting).
  - Back-face culling disabled (double-sided) to match legacy.
  - MSAA: uses `ViewBuilder::main_target_default_msaa_state(ctx.render_config(), false)` (same as other pipelines, likely 4x).
  - Vertex buffer for cube rebuilt to match the old procedural layout (`box_quad.wgsl`): same corner mapping, face ordering, and vertex order.

- **Shader:** `crates/viewer/re_renderer/shader/box_cloud.wgsl`
  - Vertex: scales unit cube by half_size, translates, transforms by world_from_obj, computes world normal from vertex normal; **color currently decoded** with `linear_from_srgba(instance.color)` before passing to fragment. (We have also tried pass-through.)
  - Fragment: shading disabled by default (flag off); returns `vec4(in.color.rgb * shading, in.color.a)`; with shading off, `shading = 1.0`.

- **Visualizer / Test:**
  - `crates/viewer/re_view_spatial/tests/boxes3d_fast_path.rs` draws a cube of colored boxes (1000 instances) and checks snapshot `boxes3d_fast_path.png`.
  - Fast-path counter increments (fast path active).
  - Fill mode set to Solid in test.

## What was tried (code-only) and outcome
1) **Shading disabled by default** → cube still mismatches.
2) **Vertex color handling**: tried pass-through vs `linear_from_srgba(instance.color)` → mismatch persists.
3) **Vertex order**: rebuilt vertex buffer to match legacy procedural vertex/triangle order from `box_quad.wgsl` → mismatch persists.
4) **Culling**: disabled back-face culling → mismatch persists.
5) **Snapshot comparison**: differences confined to cube region; background/grid match.

## Observed diffs
- Diff bbox ≈ x: 357–442, y: 253–349 (cube only).
- Diff pixel count ~2.2k–2.8k depending on color handling.
- Primary color counts differ (baseline vs current):
  - Baseline: R=1659, G=1568, B=1580 pixels.
  - Current: R≈1320 each (fewer bright primaries).
- Visual: cube appears slightly darker/different coverage; background identical.

## Likely causes to investigate
1) **sRGB handling for vertex colors:** Old path sampled `Rgba8UnormSrgb` textures in vertex shader (implicit sRGB → linear conversion). New path uses vertex attributes `Unorm8x4`; no automatic sRGB in vertex stage. The current shader decodes sRGB, but the counts suggest color/coverage still differ. Try:
   - Store linear colors in the instance buffer (float), or
   - Treat color as *un*decoded sRGB in shader and ensure the render target conversion matches the old path.

2) **MSAA / coverage differences:** Procedural path may have had slightly different triangle coverage. Try forcing the box pipeline to MSAA=1 (no MSAA) to see if coverage matches snapshot; or align MSAA settings to whatever the old path used.

3) **Color packing / gamma:** Ensure we are not double-decoding or missing decode. The current path decodes sRGBA; verify whether the legacy path effectively wrote linear or sRGB to the render target. The main target format is `Rgba8UnormSrgb`; fragment outputs expect linear.

4) **Winding / face order:** Vertex order now matches legacy; cull off. Verify winding isn’t inverted on any faces versus the old procedural generation.

## Repro
```
RUSTC_WRAPPER= cargo test -p re_view_spatial --test boxes3d_fast_path
```
Diff: `crates/viewer/re_view_spatial/tests/snapshots/boxes3d_fast_path.diff.png`

## Files touched during attempts
- `crates/viewer/re_renderer/src/renderer/box_cloud.rs` (flags default, vertex order, cull mode)
- `crates/viewer/re_renderer/shader/box_cloud.wgsl` (color decode)
- Snapshot file currently differs (`crates/viewer/re_view_spatial/tests/snapshots/boxes3d_fast_path.png`) but not intended to be updated.

## Suggested next experiments
1) **Color as linear floats**: write instance colors as linear `Float32x4` to vertex buffer; in shader, use pass-through (no sRGB decode). Compare snapshot.
2) **Disable MSAA for box pipeline**: set multisample to 1 sample for this pipeline only; compare snapshot.
3) **Force sRGB pass-through**: treat `instance.color` as already-linear (no decode) and confirm if primary counts align; or explicitly encode/decode to mimic texture sampling (simulate sampling from an sRGB texture in vertex shader).
4) **Side-by-side diff**: render both paths (legacy procedural vs instanced) in one frame and capture CPU-side diff to isolate per-pixel delta.

## Status
Fast path functional but snapshot mismatch remains localized to the cube. No snapshot updates have been committed; code changes remain uncommitted pending a decision.
