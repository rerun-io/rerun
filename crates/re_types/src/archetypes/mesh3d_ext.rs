use crate::{components::TriangleIndices, datatypes::UVec3D};

use super::Mesh3D;

#[derive(thiserror::Error, Debug)]
pub enum Mesh3DError {
    #[error("No indices were specified, so the number of positions must be divisible by 9 [(xyz xyz xyz), …], got {0}")]
    PositionsAreNotTriangles(usize),

    #[error("Index out of bounds: got index={index} with {num_vertices} vertices")]
    IndexOutOfBounds { index: u32, num_vertices: usize },

    #[error(
        "Positions & normals array must have the same length, \
        got positions={0} vs. normals={1}"
    )]
    MismatchedPositionsNormals(usize, usize),
}

impl Mesh3D {
    /// Check that this is a valid mesh, e.g. that the vertex indices are within bounds
    /// and that we have the same number of positions and normals (if any).
    pub fn sanity_check(&self) -> Result<(), Mesh3DError> {
        let num_vertices = self.num_vertices();

        if let Some(indices) = self.triangle_indices.as_ref() {
            for &TriangleIndices(UVec3D([x, y, z])) in indices {
                if num_vertices <= x as usize {
                    return Err(Mesh3DError::IndexOutOfBounds {
                        index: x,
                        num_vertices,
                    });
                }
                if num_vertices <= y as usize {
                    return Err(Mesh3DError::IndexOutOfBounds {
                        index: y,
                        num_vertices,
                    });
                }
                if num_vertices <= z as usize {
                    return Err(Mesh3DError::IndexOutOfBounds {
                        index: z,
                        num_vertices,
                    });
                }
            }
        } else if self.vertex_positions.len() % 9 != 0 {
            return Err(Mesh3DError::PositionsAreNotTriangles(
                self.vertex_positions.len(),
            ));
        }

        if let Some(normals) = &self.vertex_normals {
            if normals.len() != self.vertex_positions.len() {
                return Err(Mesh3DError::MismatchedPositionsNormals(
                    self.vertex_positions.len(),
                    normals.len(),
                ));
            }
        }

        Ok(())
    }

    /// The total number of vertices.
    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.vertex_positions.len()
    }

    /// The total number of triangles.
    #[inline]
    pub fn num_triangles(&self) -> usize {
        if let Some(indices) = self.triangle_indices.as_ref() {
            indices.len()
        } else {
            self.num_vertices() / 3
        }
    }
}
