mod geo_line_strings;
mod geo_points;
mod geo_polygons;
mod polygon_geometry;

pub use geo_line_strings::{GeoLineStringsOutput, GeoLineStringsVisualizer};
pub use geo_points::{GeoPointsOutput, GeoPointsVisualizer};
pub use geo_polygons::{GeoPolygonsDrawCache, GeoPolygonsOutput, GeoPolygonsVisualizer};

/// Helper to track an area span in latitude and longitude.
#[derive(Debug, Clone)]
pub struct GeoSpan {
    pub min_latitude: f64,
    pub max_latitude: f64,
    pub min_longitude: f64,
    pub max_longitude: f64,
}

impl GeoSpan {
    pub fn from_lat_long(mut lat_lon: impl Iterator<Item = (f64, f64)>) -> Option<Self> {
        if let Some((lat, lon)) = lat_lon.next() {
            let mut span = Self {
                min_latitude: lat,
                max_latitude: lat,
                min_longitude: lon,
                max_longitude: lon,
            };

            for (lat, lon) in lat_lon {
                span.min_latitude = span.min_latitude.min(lat);
                span.max_latitude = span.max_latitude.max(lat);
                span.min_longitude = span.min_longitude.min(lon);
                span.max_longitude = span.max_longitude.max(lon);
            }

            Some(span)
        } else {
            None
        }
    }

    pub fn center(&self) -> walkers::Position {
        let projected_latitude = f64::midpoint(
            Self::latitude_to_mercator(self.min_latitude),
            Self::latitude_to_mercator(self.max_latitude),
        );
        let latitude = ((projected_latitude - 0.5) * std::f64::consts::TAU)
            .sinh()
            .atan()
            .to_degrees();
        walkers::lat_lon(
            latitude,
            f64::midpoint(self.min_longitude, self.max_longitude),
        )
    }

    fn latitude_to_mercator(latitude: f64) -> f64 {
        let latitude_rad = latitude.to_radians();
        f64::midpoint(1.0, latitude_rad.tan().asinh() / std::f64::consts::PI).clamp(0.0, 1.0)
    }

    pub fn zoom_for_screen_size(&self, screen_size: egui::Vec2) -> Option<f64> {
        // Thanks, Claude: https://claude.site/artifacts/cb4f7f53-07a6-4ad0-bce3-eee3cb7e3177

        if self.min_latitude == self.max_latitude || self.min_longitude == self.max_longitude {
            return None;
        }

        //TODO(ab): should use the actual tile size from the map provider (always 256 in practice)
        const TILE_SIZE: f64 = 256.0;

        // Calculate ranges
        let lat_range = Self::latitude_to_mercator(self.max_latitude)
            - Self::latitude_to_mercator(self.min_latitude);
        let lon_range = (self.max_longitude - self.min_longitude) / 360.0;

        // Calculate the required number of tiles for both dimensions
        let tiles_x = lon_range * TILE_SIZE;
        let tiles_y = lat_range * TILE_SIZE;

        // Calculate zoom levels needed for both dimensions
        let zoom_x = (screen_size.x as f64 / tiles_x).ln() / 2.0_f64.ln();
        let zoom_y = (screen_size.y as f64 / tiles_y).ln() / 2.0_f64.ln();

        // Use the minimum zoom level to ensure the entire range fits
        Some(zoom_x.min(zoom_y))
    }
}

/// Extend a span to include another span, if any.
pub fn update_span(span: &mut Option<GeoSpan>, other: Option<GeoSpan>) {
    if let Some(other) = other {
        match span {
            Some(span) => {
                span.min_latitude = span.min_latitude.min(other.min_latitude);
                span.max_latitude = span.max_latitude.max(other.max_latitude);
                span.min_longitude = span.min_longitude.min(other.min_longitude);
                span.max_longitude = span.max_longitude.max(other.max_longitude);
            }
            None => *span = Some(other),
        }
    }
}

/// Convert a [`re_sdk_types::components::Radius`] to a [`re_renderer::Size`], considering scene units
/// as meters.
#[inline]
pub fn radius_to_size(
    radius: re_sdk_types::components::Radius,
    projector: &walkers::Projector,
    position: walkers::Position,
) -> re_renderer::Size {
    re_renderer::Size(
        radius
            .scene_units()
            .map(|radius_meter| projector.scale_pixel_per_meter(position) * radius_meter)
            .or_else(|| radius.ui_points())
            .unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_framing_contains_asymmetric_mercator_bounds() {
        for (min_latitude, max_latitude) in [(0.0, 80.0), (-85.0, 83.6), (-70.0, -10.0)] {
            let span = GeoSpan {
                min_latitude,
                max_latitude,
                min_longitude: -10.0,
                max_longitude: 10.0,
            };
            for size in [egui::vec2(1024.0, 500.0), egui::vec2(500.0, 900.0)] {
                let rect = egui::Rect::from_min_size(egui::pos2(17.0, 23.0), size);
                let mut memory = walkers::MapMemory::default();
                memory
                    .set_zoom(span.zoom_for_screen_size(size).unwrap())
                    .unwrap();
                let projector = walkers::Projector::new(rect, &memory, span.center());
                for latitude in [span.min_latitude, span.max_latitude] {
                    for longitude in [span.min_longitude, span.max_longitude] {
                        let position = projector.project(walkers::lat_lon(latitude, longitude));
                        assert!(
                            rect.expand(0.01).contains(position.to_pos2()),
                            "Automatic framing clipped ({latitude}, {longitude}) at {position:?}: {span:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_zoom_for_screen_size() {
        // smaller area
        let span = GeoSpan {
            min_latitude: 46.0,
            max_latitude: 47.0,
            min_longitude: 6.0,
            max_longitude: 7.0,
        };

        let zoom = span
            .zoom_for_screen_size(egui::Vec2::new(1024.0, 500.0))
            .unwrap();
        assert!(zoom >= 8.0 && zoom <= 9.0);

        // whole world
        let span = GeoSpan {
            min_latitude: -85.0,
            max_latitude: 85.0,
            min_longitude: -180.0,
            max_longitude: 180.0,
        };

        let zoom = span
            .zoom_for_screen_size(egui::Vec2::new(1024.0, 512.0))
            .unwrap();
        assert!(zoom >= 0.0 && zoom <= 2.0); // Should be very zoomed out

        // single point
        let span = GeoSpan {
            min_latitude: 46.0,
            max_latitude: 46.0,
            min_longitude: 6.0,
            max_longitude: 6.0,
        };
        assert_eq!(
            None,
            span.zoom_for_screen_size(egui::Vec2::new(1024.0, 512.0))
        );
    }
}
