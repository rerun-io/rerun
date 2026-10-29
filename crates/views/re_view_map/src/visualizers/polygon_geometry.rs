//! View-independent polygon preparation. Screen projection happens after triangulation.

pub const MAX_MERCATOR_LATITUDE: f64 = 85.051_128_779_806_6;

#[derive(Clone, Debug)]
pub struct PolygonGeometry {
    pub lat_lon: Vec<[f64; 2]>,
    pub triangles: Vec<[u32; 3]>,
}

fn mercator([lat, lon]: [f64; 2]) -> [f64; 2] {
    [
        lon / 360.0,
        lat.to_radians().tan().asinh() / std::f64::consts::TAU,
    ]
}

fn ring_area(ring: &[[f64; 2]]) -> f64 {
    let origin = ring[0];
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| {
            (a[0] - origin[0]) * (b[1] - origin[1]) - (a[1] - origin[1]) * (b[0] - origin[0])
        })
        .sum::<f64>()
        .abs()
        * 0.5
}

/// Prepare valid, non-self-intersecting rings for the map's Mercator projection.
///
/// Unsplit antimeridian crossings are rejected instead of drawing a region across the world.
/// This does not repair invalid polygon topology.
pub fn triangulate(
    lat_lon: &[[f64; 2]],
    ring_lengths: &[u64],
) -> Result<PolygonGeometry, &'static str> {
    if ring_lengths.is_empty() {
        return Err("A polygon requires an exterior ring");
    }
    // Earcut uses u32 byte offsets for its internal nodes, including nodes added for holes.
    if lat_lon.len() > 16_000_000 {
        return Err("Too many polygon vertices to triangulate");
    }
    let mut positions = Vec::with_capacity(lat_lon.len());
    let mut projected = Vec::with_capacity(lat_lon.len());
    let mut holes = Vec::with_capacity(ring_lengths.len().saturating_sub(1));
    let mut start = 0usize;
    let mut expected_area = 0.0;
    for (ring_index, &length) in ring_lengths.iter().enumerate() {
        let length = usize::try_from(length).map_err(|_err| "Polygon ring length overflow")?;
        let end = start
            .checked_add(length)
            .ok_or("Polygon ring length overflow")?;
        let ring = lat_lon
            .get(start..end)
            .ok_or("Polygon ring lengths exceed the vertex count")?;
        start = end;
        let first = positions.len();
        for &[lat, lon] in ring {
            if !lat.is_finite() || !lon.is_finite() {
                return Err("Polygon coordinates must be finite");
            }
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                return Err("Polygon latitude/longitude is outside EPSG:4326 bounds");
            }
            let position = [
                lat.clamp(-MAX_MERCATOR_LATITUDE, MAX_MERCATOR_LATITUDE),
                lon,
            ];
            if positions.len() == first || positions.last() != Some(&position) {
                positions.push(position);
            }
        }
        if positions.len() > first + 1 && positions.last() == positions.get(first) {
            positions.pop();
        }
        let ring = &positions[first..];
        if ring.len() < 3 {
            return Err("A polygon ring requires at least three distinct vertices");
        }
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            if (a[1] - b[1]).abs() > 180.0 {
                return Err("Split polygons at the antimeridian before logging");
            }
        }
        if ring_index > 0 {
            holes.push(first as u32);
        }
        projected.extend(ring.iter().copied().map(mercator));
        let area = ring_area(&projected[first..]);
        if area == 0.0 {
            return Err("A polygon ring has zero area");
        }
        expected_area += if ring_index == 0 { area } else { -area };
    }
    if start != lat_lon.len() {
        return Err("Polygon ring lengths do not cover the vertex count");
    }
    if expected_area <= 0.0 {
        return Err("Polygon holes cover the exterior ring");
    }
    let mut indices = Vec::<u32>::new();
    earcut::Earcut::new().earcut(projected.iter().copied(), &holes, &mut indices);
    let triangles = indices.as_chunks::<3>().0.to_vec();
    let actual_area: f64 = triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|index| projected[index as usize]);
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
        })
        .sum();
    if triangles.is_empty() || (actual_area - expected_area).abs() > expected_area * 1e-8 {
        return Err("Unable to triangulate polygon rings; check polygon topology");
    }
    Ok(PolygonGeometry {
        lat_lon: positions,
        triangles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_polygon_with_hole_accepts_both_windings() {
        let outer = [
            [0.0, 0.0],
            [0.0, 6.0],
            [2.0, 6.0],
            [2.0, 2.0],
            [6.0, 2.0],
            [6.0, 0.0],
        ];
        let hole = [[0.5, 0.5], [0.5, 1.5], [1.5, 1.5], [1.5, 0.5]];
        for reversed in [false, true] {
            let mut rings = [outer.to_vec(), hole.to_vec()];
            if reversed {
                for ring in &mut rings {
                    ring.reverse();
                }
            }
            let polygon = triangulate(&rings.concat(), &[6, 4]).unwrap();
            assert_eq!(polygon.lat_lon.len(), 10);
            assert_eq!(polygon.triangles.len(), 10);
        }
    }

    #[test]
    fn closure_and_consecutive_duplicates_are_removed() {
        let polygon = triangulate(
            &[[0.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            &[5],
        )
        .unwrap();
        assert_eq!(polygon.lat_lon.len(), 3);
        assert_eq!(polygon.triangles.len(), 1);
    }

    #[test]
    fn malformed_data_is_rejected_without_panicking() {
        let triangle = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]];
        for lengths in [&[][..], &[2][..], &[4][..], &[u64::MAX][..], &[3, 0][..]] {
            assert!(triangulate(&triangle, lengths).is_err());
        }
        for invalid in [
            [f64::NAN, 0.0],
            [f64::INFINITY, 0.0],
            [91.0, 0.0],
            [0.0, 181.0],
        ] {
            assert!(triangulate(&[triangle[0], triangle[1], invalid], &[3]).is_err());
        }
        assert!(triangulate(&[[0.0, 0.0], [0.0, 1.0], [0.0, 2.0]], &[3]).is_err());
    }

    #[test]
    fn unsplit_antimeridian_crossing_is_diagnosed() {
        let result = triangulate(&[[0.0, 179.0], [1.0, -179.0], [1.0, 179.0]], &[3]);
        assert!(result.unwrap_err().contains("antimeridian"));
    }

    #[test]
    fn polar_latitudes_remain_finite() {
        let polygon = triangulate(&[[84.0, 0.0], [90.0, 1.0], [84.0, 2.0]], &[3]).unwrap();
        assert_eq!(polygon.lat_lon[1][0], MAX_MERCATOR_LATITUDE);
    }
}
