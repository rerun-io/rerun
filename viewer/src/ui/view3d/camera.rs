use egui::Rect;
use glam::Affine3A;
use macaw::{vec3, IsoTransform, Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct Camera {
    pub world_from_view: IsoTransform,
    pub fov_y: f32,
}

impl Camera {
    #[allow(clippy::unused_self)]
    pub fn near(&self) -> f32 {
        0.01 // TODO
    }

    pub fn screen_from_world(&self, rect: &Rect) -> Mat4 {
        let aspect_ratio = rect.width() / rect.height();
        Mat4::from_translation(vec3(rect.center().x, rect.center().y, 0.0))
            * Mat4::from_scale(0.5 * vec3(rect.width(), -rect.height(), 1.0))
            * Mat4::perspective_infinite_rh(self.fov_y, aspect_ratio, self.near())
            * self.world_from_view.inverse()
    }

    pub fn pos(&self) -> glam::Vec3 {
        self.world_from_view.translation()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct OrbitCamera {
    pub center: Vec3,
    pub radius: f32,
    pub world_from_view_rot: Quat,
    pub fov_y: f32,
    /// Zero = no up (3dof rotation)
    pub up: Vec3,
}

impl OrbitCamera {
    const MAX_PITCH: f32 = 0.999 * 0.25 * std::f32::consts::TAU;

    pub fn to_camera(self) -> Camera {
        let pos = self.center + self.world_from_view_rot * vec3(0.0, 0.0, self.radius);
        Camera {
            world_from_view: IsoTransform::from_rotation_translation(self.world_from_view_rot, pos),
            fov_y: self.fov_y,
        }
    }

    /// Direction we are looking at
    pub fn dir(&self) -> Vec3 {
        self.world_from_view_rot * -Vec3::Z
    }

    /// Only valid if we have an up vector.
    ///
    /// `[-tau/4, +tau/4]`
    fn pitch(&self) -> Option<f32> {
        if self.up == Vec3::ZERO {
            None
        } else {
            Some(self.dir().dot(self.up).clamp(-1.0, 1.0).asin())
        }
    }

    fn set_dir(&mut self, dir: Vec3) {
        if self.up == Vec3::ZERO {
            self.world_from_view_rot = Quat::from_rotation_arc(-Vec3::Z, dir);
        } else {
            let pitch = self
                .pitch()
                .unwrap()
                .clamp(-Self::MAX_PITCH, Self::MAX_PITCH);

            let dir = project_onto(dir, self.up).normalize(); // Remove pitch
            let right = dir.cross(self.up).normalize();
            let dir = Quat::from_axis_angle(right, pitch) * dir; // Tilt up/down
            let dir = dir.normalize(); // Prevent drift

            self.world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, dir, self.up).inverse());
        }
    }

    pub fn set_up(&mut self, up: Vec3) {
        self.up = up.normalize_or_zero();

        if self.up != Vec3::ZERO {
            self.set_dir(self.dir()); // this will clamp the rotation
        }
    }

    /// Rotate based on a certain number of pixel delta.
    pub fn rotate(&mut self, delta: egui::Vec2) {
        let sensitivity = 0.004; // radians-per-point  TODO: take fov_y and canvas size into account
        let delta = sensitivity * delta;

        if self.up == Vec3::ZERO {
            // 3-dof rotation
            let rot_delta = Quat::from_rotation_y(-delta.x) * Quat::from_rotation_x(-delta.y);
            self.world_from_view_rot *= rot_delta;
        } else {
            // 2-dof rotation
            let dir = Quat::from_axis_angle(self.up, -delta.x) * self.dir();
            let dir = dir.normalize(); // Prevent drift

            let pitch = self.pitch().unwrap() - delta.y;
            let pitch = pitch.clamp(-Self::MAX_PITCH, Self::MAX_PITCH);

            let dir = project_onto(dir, self.up).normalize(); // Remove pitch
            let right = dir.cross(self.up).normalize();
            let dir = Quat::from_axis_angle(right, pitch) * dir; // Tilt up/down
            let dir = dir.normalize(); // Prevent drift

            self.world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, dir, self.up).inverse());
        }
    }
}

/// e.g. up is [0,0,1], we return things like [x,y,0]
fn project_onto(v: Vec3, up: Vec3) -> Vec3 {
    v - up * v.dot(up)
}
