/// Utilities for generating box vertices procedurally in the vertex shader.
///
/// Each box is rendered as 12 triangles (2 per face × 6 faces) = 36 vertices.
/// The vertex shader receives a vertex_index from 0 to 35 for each box instance.
///
/// This approach allows us to store only box center + half_size data on the GPU,
/// generating the full vertex positions and normals procedurally.

/// Returns the index of the current box (which box instance we're rendering).
///
/// vertex_idx: The global vertex index from the vertex shader
/// Returns: The box index (0 for first box, 1 for second box, etc.)
fn box_index(vertex_idx: u32) -> u32 {
    return vertex_idx / 36u; // 36 vertices per box
}

/// Returns the position of a vertex within a unit box centered at origin.
///
/// The box extends from [-0.5, -0.5, -0.5] to [0.5, 0.5, 0.5].
/// This will be scaled by the box's half_size and translated to its center position.
///
/// vertex_idx: The global vertex index from the vertex shader
/// Returns: Position within the unit box
fn box_vertex_position(vertex_idx: u32) -> vec3f {
    let local_idx = vertex_idx % 36u;
    let face_idx = local_idx / 6u;  // Which face (0-5)
    let tri_vert_idx = local_idx % 6u; // Which vertex within the quad (0-5)

    // Each face is a quad made of 2 triangles:
    // Vertices 0,1,2 make the first triangle
    // Vertices 3,4,5 make the second triangle
    // Layout: 0--1    where triangles are 0-1-2 and 2-3-0 (rewired as 3-4-5)
    //         |  |
    //         3--2
    //
    // We map tri_vert_idx to corners: 0 -> 0, 1 -> 1, 2 -> 2, 3 -> 2, 4 -> 3, 5 -> 0
    var corner_idx: u32;
    if tri_vert_idx == 0u {
        corner_idx = 0u;
    } else if tri_vert_idx == 1u {
        corner_idx = 1u;
    } else if tri_vert_idx == 2u || tri_vert_idx == 3u {
        corner_idx = 2u;
    } else if tri_vert_idx == 4u {
        corner_idx = 3u;
    } else { // tri_vert_idx == 5u
        corner_idx = 0u;
    }

    // Define the 4 corners of each face
    // Each face is defined by which dimension is constant and what the 4 corners are
    var pos: vec3f;

    if face_idx == 0u {
        // Front face (+Z)
        // Corners: (-0.5, -0.5, 0.5), (0.5, -0.5, 0.5), (0.5, 0.5, 0.5), (-0.5, 0.5, 0.5)
        let x = select(0.5, -0.5, corner_idx == 0u || corner_idx == 3u);
        let y = select(0.5, -0.5, corner_idx == 0u || corner_idx == 1u);
        pos = vec3f(x, y, 0.5);
    } else if face_idx == 1u {
        // Back face (-Z)
        // Corners: (0.5, -0.5, -0.5), (-0.5, -0.5, -0.5), (-0.5, 0.5, -0.5), (0.5, 0.5, -0.5)
        let x = select(-0.5, 0.5, corner_idx == 0u || corner_idx == 3u);
        let y = select(0.5, -0.5, corner_idx == 0u || corner_idx == 1u);
        pos = vec3f(x, y, -0.5);
    } else if face_idx == 2u {
        // Right face (+X)
        // Corners: (0.5, -0.5, 0.5), (0.5, -0.5, -0.5), (0.5, 0.5, -0.5), (0.5, 0.5, 0.5)
        let z = select(-0.5, 0.5, corner_idx == 0u || corner_idx == 3u);
        let y = select(0.5, -0.5, corner_idx == 0u || corner_idx == 1u);
        pos = vec3f(0.5, y, z);
    } else if face_idx == 3u {
        // Left face (-X)
        // Corners: (-0.5, -0.5, -0.5), (-0.5, -0.5, 0.5), (-0.5, 0.5, 0.5), (-0.5, 0.5, -0.5)
        let z = select(0.5, -0.5, corner_idx == 0u || corner_idx == 3u);
        let y = select(0.5, -0.5, corner_idx == 0u || corner_idx == 1u);
        pos = vec3f(-0.5, y, z);
    } else if face_idx == 4u {
        // Top face (+Y)
        // Corners: (-0.5, 0.5, 0.5), (0.5, 0.5, 0.5), (0.5, 0.5, -0.5), (-0.5, 0.5, -0.5)
        let x = select(0.5, -0.5, corner_idx == 0u || corner_idx == 3u);
        let z = select(-0.5, 0.5, corner_idx == 0u || corner_idx == 1u);
        pos = vec3f(x, 0.5, z);
    } else { // face_idx == 5u
        // Bottom face (-Y)
        // Corners: (-0.5, -0.5, -0.5), (0.5, -0.5, -0.5), (0.5, -0.5, 0.5), (-0.5, -0.5, 0.5)
        let x = select(0.5, -0.5, corner_idx == 0u || corner_idx == 3u);
        let z = select(0.5, -0.5, corner_idx == 0u || corner_idx == 1u);
        pos = vec3f(x, -0.5, z);
    }

    return pos;
}

/// Returns the normal vector for a given vertex.
///
/// All vertices on the same face share the same normal.
///
/// vertex_idx: The global vertex index from the vertex shader
/// Returns: The unit normal vector for this face
fn box_vertex_normal(vertex_idx: u32) -> vec3f {
    let face_idx = (vertex_idx % 36u) / 6u;

    if face_idx == 0u {
        return vec3f(0.0, 0.0, 1.0);  // Front face (+Z)
    } else if face_idx == 1u {
        return vec3f(0.0, 0.0, -1.0); // Back face (-Z)
    } else if face_idx == 2u {
        return vec3f(1.0, 0.0, 0.0);  // Right face (+X)
    } else if face_idx == 3u {
        return vec3f(-1.0, 0.0, 0.0); // Left face (-X)
    } else if face_idx == 4u {
        return vec3f(0.0, 1.0, 0.0);  // Top face (+Y)
    } else { // face_idx == 5u
        return vec3f(0.0, -1.0, 0.0); // Bottom face (-Y)
    }
}
