# Raw Mesh Example Refactoring Plan

## Issue Summary

**GitHub Issue**: [#1957](https://github.com/rerun-io/rerun/issues/1957)
**Title**: "raw_mesh example is misleading in that it loads gltf files"

### Problem Statement

The current `raw_mesh` examples (Python and Rust) are misleading because they:
1. Load GLTF/GLB files to demonstrate mesh functionality
2. GLTF/GLB files can already be loaded natively via Rerun's `Asset3D` archetype
3. The examples don't actually demonstrate how to construct **raw mesh data** from scratch
4. This creates confusion about the purpose and capabilities of the `Mesh3D` archetype vs `Asset3D`

Both README files already acknowledge this issue with a TODO comment:
```html
<!-- TODO(#1957): How about we load something elseto avoid confusion? -->
```

### Current Affected Files

**Python Implementation:**
- `examples/python/raw_mesh/raw_mesh/__main__.py`
- `examples/python/raw_mesh/README.md`
- `examples/python/raw_mesh/raw_mesh/download_dataset.py` (can be removed)

**Rust Implementation:**
- `examples/rust/raw_mesh/src/main.rs`
- `examples/rust/raw_mesh/README.md`

## Proposed Solution

Replace the GLTF file loading with **procedurally generated geometric primitives** that demonstrate:
1. How to construct mesh vertices, normals, and indices from scratch
2. How to apply colors and materials programmatically
3. How to create a transform hierarchy without external dependencies
4. The full capabilities of the `Mesh3D` archetype

## Implementation Details

### Geometric Primitives to Generate

Create a showcase of common 3D primitives, each demonstrating different `Mesh3D` features:

1. **Colored Cube**
   - Demonstrates: Basic vertex positions, triangle indices, per-vertex colors
   - 8 vertices, 12 triangles (6 faces × 2 triangles each)
   - Each face with a different color

2. **Textured Pyramid**
   - Demonstrates: UV texture coordinates, albedo texture
   - 5 vertices (4 base + 1 apex), 6 triangles
   - Procedurally generated checkerboard texture

3. **Smooth Sphere**
   - Demonstrates: Vertex normals for smooth shading, uniform albedo factor
   - UV sphere topology (latitude/longitude grid)
   - ~1000-2000 triangles for smooth appearance

4. **Flat-Shaded Icosahedron**
   - Demonstrates: Flat shading (no normals or face normals)
   - 12 vertices, 20 triangles
   - Simple platonic solid geometry

5. **Grid Mesh**
   - Demonstrates: Transform hierarchy (multiple instances at different positions)
   - Simple XZ plane grid
   - Multiple instances with different transforms

### Hierarchical Scene Structure

Create a hierarchical scene to demonstrate `Transform3D`:

```
world/
├── primitives/
│   ├── cube (at origin)
│   ├── pyramid (translated +X)
│   ├── sphere (translated -X)
│   ├── icosahedron (translated +Y)
│   └── grid_instances/
│       ├── grid_0 (translated +Z, rotated)
│       ├── grid_1 (translated -Z, scaled)
│       └── grid_2 (rotated differently)
```

### Python Implementation Changes

**File: `examples/python/raw_mesh/raw_mesh/__main__.py`**

Changes needed:
1. Remove `trimesh` dependency
2. Remove GLTF loading logic
3. Add geometric primitive generation functions:
   - `generate_cube()` → vertices, indices, colors
   - `generate_pyramid()` → vertices, indices, UV coords, texture
   - `generate_sphere(subdivisions)` → vertices, indices, normals
   - `generate_icosahedron()` → vertices, indices
   - `generate_grid()` → vertices, indices
4. Add procedural texture generation (simple checkerboard pattern)
5. Update scene logging to use generated meshes
6. Update argument parser (remove scene selection, add options like `--subdivisions`)
7. Update blueprint and description

**File: `examples/python/raw_mesh/pyproject.toml`**

Changes needed:
1. Remove `trimesh` dependency
2. Keep only `numpy` and `rerun-sdk`

**File: `examples/python/raw_mesh/README.md`**

Changes needed:
1. Update description to emphasize procedural generation
2. Remove references to GLTF files and scenes
3. Update code examples to show primitive generation
4. Remove the TODO comment
5. Update run instructions (no more `--scene` option)

**File: `examples/python/raw_mesh/raw_mesh/download_dataset.py`**

Changes needed:
1. Delete this file entirely (no longer needed)

### Rust Implementation Changes

**File: `examples/rust/raw_mesh/src/main.rs`**

Changes needed:
1. Remove GLTF parsing code (lines 179-303)
2. Remove `gltf` and `bytes` dependencies
3. Add geometric primitive generation functions:
   - `generate_cube()` → `GltfPrimitive` struct (reuse for primitives)
   - `generate_pyramid()` → with texture
   - `generate_sphere(subdivisions)` → with normals
   - `generate_icosahedron()`
   - `generate_grid()`
4. Rename `GltfPrimitive`, `GltfNode`, `GltfTransform` to generic names:
   - `GltfPrimitive` → `MeshPrimitive`
   - `GltfNode` → `MeshNode`
   - `GltfTransform` → `MeshTransform`
5. Update `run()` function to create procedural scene
6. Remove command-line scene selection
7. Add procedural texture generation

**File: `examples/rust/raw_mesh/Cargo.toml`**

Changes needed:
1. Remove `gltf` dependency
2. Remove `bytes` dependency
3. Keep `rerun` and basic dependencies

**File: `examples/rust/raw_mesh/README.md`**

Changes needed:
1. Update description to emphasize procedural generation
2. Remove references to GLTF files
3. Remove the TODO comment
4. Update run instructions

### Documentation Updates

Both implementations should include:
1. Inline code comments explaining the math/geometry
2. Comments showing how to calculate normals
3. Comments about winding order (counter-clockwise for front faces)
4. Examples of different color/material approaches
5. Links to `Asset3D` for users who want to load mesh files

### Testing Considerations

1. Visual regression tests may need updating (screenshots will change)
2. Example metadata in README frontmatter should be updated
3. Thumbnails will need regeneration

## Benefits of This Approach

1. **Educational**: Shows users exactly how to construct mesh data from scratch
2. **Self-contained**: No external file dependencies or downloads
3. **Clear purpose**: Distinguishes `Mesh3D` (programmatic) from `Asset3D` (file-based)
4. **Comprehensive**: Demonstrates all `Mesh3D` features in one example
5. **Fast**: No file I/O, runs instantly
6. **Platform-independent**: Pure math, no file path issues

## Backward Compatibility

Since these are examples (not API), backward compatibility is not a concern. However:
1. Users who reference this example in documentation should be notified
2. The old thumbnails/screenshots should be replaced
3. Any tutorials referencing the GLTF scenes should be updated

## Implementation Steps

1. **Python implementation first** (easier to prototype)
   - Implement geometric primitive generators
   - Test each primitive individually
   - Create hierarchical scene
   - Update documentation

2. **Rust implementation** (based on Python approach)
   - Port geometric algorithms to Rust
   - Maintain same visual output as Python version
   - Update documentation

3. **Documentation & assets**
   - Generate new screenshots/thumbnails
   - Update README files
   - Remove dataset download logic

4. **Testing**
   - Visual verification of all primitives
   - Verify both Python and Rust produce similar output
   - Test on different platforms

## Open Questions

1. **Texture generation**: Should we generate actual image textures or use solid colors?
   - Recommendation: Simple procedural checkerboard (demonstrates UV mapping without image files)

2. **Complexity**: How detailed should the sphere be?
   - Recommendation: Make it configurable, default ~32 latitude × 16 longitude segments

3. **Additional features**: Should we demonstrate:
   - Multiple materials per mesh? (No - keep it simple)
   - Texture arrays? (No - too advanced)
   - Line rendering? (No - different archetype)

## Success Criteria

- [ ] No external file dependencies (GLTF, images, etc.)
- [ ] Demonstrates all `Mesh3D` archetype features
- [ ] Clear, well-commented code showing geometric construction
- [ ] Both Python and Rust implementations produce similar visual output
- [ ] Documentation clearly explains the difference between `Mesh3D` and `Asset3D`
- [ ] Examples run instantly without downloads
- [ ] Issue #1957 can be closed

## Estimated Effort

- Python implementation: 4-6 hours
- Rust implementation: 4-6 hours
- Documentation & testing: 2-3 hours
- **Total**: 10-15 hours

## References

- Issue: https://github.com/rerun-io/rerun/issues/1957
- `Mesh3D` docs: https://rerun.io/docs/reference/types/archetypes/mesh3d
- `Asset3D` docs: https://rerun.io/docs/reference/types/archetypes/asset3d
- PR #10249 (partial fix): Linked to `Asset3D` in documentation
