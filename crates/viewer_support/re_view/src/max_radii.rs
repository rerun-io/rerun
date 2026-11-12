use re_sdk_types::components::Radius;

/// The maximum point radii, tracking both the ui point & scene unit maxima separately.
#[derive(Clone, Copy, Debug, Default, PartialEq, re_byte_size::SizeBytes)]
pub struct MaxRadii {
    ui_points: Option<f32>,
    scene_units: Option<f32>,
}

impl MaxRadii {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ui_points(&self) -> Option<f32> {
        self.ui_points
    }
    pub fn scene_units(&self) -> Option<f32> {
        self.scene_units
    }

    pub fn merge(&mut self, other: Self) {
        *self = Self {
            ui_points: [self.ui_points, other.ui_points]
                .into_iter()
                .flatten()
                .reduce(f32::max),
            scene_units: [self.scene_units, other.scene_units]
                .into_iter()
                .flatten()
                .reduce(f32::max),
        }
    }

    pub fn update(&mut self, radius: Radius) {
        if let Some(screen) = radius.ui_points() {
            self.ui_points
                .replace(self.ui_points.map(|s| s.max(screen)).unwrap_or(screen));
        }
        if let Some(scene) = radius.scene_units() {
            self.scene_units
                .replace(self.scene_units.map(|s| s.max(scene)).unwrap_or(scene));
        }
    }

    pub fn update_from_radii(&mut self, radii: impl IntoIterator<Item = Radius>) {
        for radius in radii {
            self.update(radius);
        }
    }
}
