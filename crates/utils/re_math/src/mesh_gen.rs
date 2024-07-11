use std::ops::Range;

use glam::Vec3;

use crate::IsoTransform;

/// Raw mesh generator. Only generates positions, normals and an index buffer.
///
/// Composable - to create a composite mesh, just repeatedly call the various
/// generation functions. Each one will return the range of vertices added, so if you
/// have parallel arrays of things like colors, you know how many to push.
#[derive(Default)]
pub struct MeshGen {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<u32>,
}

fn transform_points(points: &mut [Vec3], transform: IsoTransform) {
    if transform != IsoTransform::IDENTITY {
        for point in points {
            *point = transform.transform_point3(*point);
        }
    }
}

fn transform_vectors(vectors: &mut [Vec3], transform: IsoTransform) {
    if transform != IsoTransform::IDENTITY {
        for vector in vectors {
            *vector = transform.transform_vector3(*vector);
        }
    }
}

impl MeshGen {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_cube(&mut self, half_size: Vec3, transform: IsoTransform) -> Range<usize> {
        #![allow(clippy::disallowed_methods)] // Use of normalize fine as long as the input is not degenerate.

        let s = half_size;

        let index_offset = self.positions.len() as u32;

        //
        //      a +--------------+ b
        //       /|             /|
        //      / |            / |
        //   c *--+-----------*  | d
        //     |  |           |  |
        //     |  |           |  |
        //     |  |           |  |
        //   e |  +-----------+--+ f
        //     | /            | /
        //     |/             |/
        //   g *--------------* h
        let mut positions = vec![
            Vec3::new(s.x, -s.y, s.z),   // g
            Vec3::new(s.x, -s.y, -s.z),  // e
            Vec3::new(s.x, s.y, -s.z),   // a
            Vec3::new(s.x, s.y, s.z),    // c
            Vec3::new(-s.x, -s.y, s.z),  // h
            Vec3::new(-s.x, -s.y, -s.z), // f
            Vec3::new(-s.x, s.y, -s.z),  // b
            Vec3::new(-s.x, s.y, s.z),   // d
        ];

        let mut indices: Vec<u32> = vec![
            4, 0, 3, 4, 3, 7, 0, 1, 2, 0, 2, 3, 1, 5, 6, 1, 6, 2, 5, 4, 7, 5, 7, 6, 7, 3, 2, 7, 2,
            6, 0, 5, 1, 0, 4, 5,
        ];

        let mut normals = {
            let face_normals: Vec<Vec3> = indices
                .chunks(3)
                .map(|i| {
                    let p = positions[i[0] as usize];
                    let a = positions[i[1] as usize] - p;
                    let b = positions[i[2] as usize] - p;
                    a.cross(b).normalize()
                })
                .collect::<Vec<_>>();

            positions = indices
                .iter()
                .map(|i| positions[*i as usize])
                .collect::<Vec<_>>();

            indices = (0..36_u32).map(|x| x + index_offset).collect::<Vec<_>>();

            indices
                .iter()
                .map(|i| face_normals[((i - index_offset) / 3) as usize])
                .collect::<Vec<Vec3>>()
        };

        let index_offset = index_offset as usize;
        let out_range = index_offset..(index_offset + positions.len());

        transform_points(&mut positions, transform);
        transform_vectors(&mut normals, transform);

        self.positions.extend(positions);
        self.normals.extend(normals);
        self.indices.extend(indices);

        out_range
    }

    /// Creates a lat-long sphere.
    pub fn push_sphere(
        &mut self,
        radius: f32,
        subdivision_x: usize,
        subdivision_y: usize,
        transform: IsoTransform,
    ) -> Range<usize> {
        self.push_capsule(radius, 0.0, subdivision_x, subdivision_y, transform)
    }

    /// Create a lat-long capsule mesh. Can also be used to create spheres
    /// by setting `length_y` = 0.
    ///
    /// Instead of coloring the sphere here, especially if you're using boxes with
    /// multiple colors, consider making it white (`ColorRgba8([255, 255, 255, 255)`)
    /// and using `MeshStyle` Tint to color the box. This applies both to the scene
    /// and world APIs.
    ///
    /// If `subdivision_x` or `subdivision_y` are less than 3 they will be overridden
    /// to 3. This is done as a lower subdivision will result in a mesh that is not
    /// visible when rendered.
    pub fn push_capsule(
        &mut self,
        radius: f32,
        length_y: f32,
        subdivision_x: usize,
        subdivision_y: usize,
        transform: IsoTransform,
    ) -> Range<usize> {
        let index_offset = self.positions.len() as u32;

        let subdivision_x = 3.max(subdivision_x as u32);
        let subdivision_y = 3.max(subdivision_y as u32);

        let delta_x = 2.0 * std::f32::consts::PI / subdivision_x as f32;
        let delta_y = std::f32::consts::PI / subdivision_y as f32;

        let mut positions = vec![];
        let mut normals = vec![];

        let middle = subdivision_y / 2;

        // North pole. Consider it negative and go towards positive.
        positions.push(Vec3::new(0.0, -radius, 0.0));
        normals.push(Vec3::new(0.0, -1.0, 0.0));

        // Stripes, including the middle
        for y in 1..subdivision_y {
            let angle_y = delta_y * y as f32;
            let y_offset = if y >= middle { length_y } else { 0.0 };
            // TODO(emilk): The middle stripe on capsules should really be a whole extra ring.
            // Still looks "good enough" with enough tessellation, but should be fixed.
            // let midstripe = y == middle || y == middle + 1;
            for x in 0..subdivision_x {
                let angle_x = delta_x * x as f32;

                let mut pos = Vec3::new(
                    angle_x.cos() * angle_y.sin(),
                    -angle_y.cos(),
                    angle_x.sin() * angle_y.sin(),
                );
                normals.push(pos);

                pos *= radius;
                pos.y += y_offset;

                positions.push(pos);
            }
        }
        // South pole.
        positions.push(Vec3::new(0.0, radius + length_y, 0.0));
        normals.push(Vec3::new(0.0, 1.0, 0.0));

        let mut indices: Vec<u32> = vec![];
        // North cap
        for i in 0..subdivision_x {
            indices.push(index_offset);
            indices.push(index_offset + 1 + i);
            indices.push(index_offset + 1 + (i + 1) % subdivision_x);
        }

        // Stripes
        for y in 0..subdivision_y - 2 {
            for x in 0..subdivision_x {
                let b = index_offset + 1 + y * subdivision_x + x;
                let b_next = index_offset + 1 + y * subdivision_x + (x + 1) % subdivision_x;
                indices.push(b);
                indices.push(b + subdivision_x);
                indices.push(b_next);
                indices.push(b_next);
                indices.push(b + subdivision_x);
                indices.push(b_next + subdivision_x);
            }
        }

        // South cap
        let b = 1 + (subdivision_y - 2) * subdivision_x + index_offset;
        for i in 0..subdivision_x {
            indices.push(b + (i + 1) % subdivision_x);
            indices.push(b + i);
            indices.push(b + subdivision_x);
        }

        let index_offset = index_offset as usize;
        let out_range = index_offset..(index_offset + positions.len());

        transform_points(&mut positions, transform);
        transform_vectors(&mut normals, transform);

        self.positions.extend(positions);
        self.normals.extend(normals);
        self.indices.extend(indices);

        out_range
    }
}
