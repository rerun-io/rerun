use egui::{NumExt, Rect};
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

    pub fn forward(&self) -> glam::Vec3 {
        self.world_from_view.rotation() * -Vec3::Z
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let translation = self
            .world_from_view
            .translation()
            .lerp(other.world_from_view.translation(), t);
        let rotation = self
            .world_from_view
            .rotation()
            .slerp(other.world_from_view.rotation(), t);
        let fov_y = egui::lerp(self.fov_y..=other.fov_y, t);
        Camera {
            world_from_view: IsoTransform::from_rotation_translation(rotation, translation),
            fov_y,
        }
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

    /// Create an [`OrbitCamera`] from a [`Camera`].
    pub fn copy_from_camera(&mut self, camera: &Camera) {
        // The hard part is finding a good center. Let's try to keep the same, and see how that goes:
        let distance = camera.forward().dot(self.center - camera.pos());
        self.radius = distance.at_least(self.radius / 5.0);
        self.center = camera.pos() + self.radius * camera.forward();
        self.world_from_view_rot = camera.world_from_view.rotation();
        self.fov_y = camera.fov_y;
    }

    /// Direction we are looking at
    fn fwd(&self) -> Vec3 {
        self.world_from_view_rot * -Vec3::Z
    }

    /// Only valid if we have an up vector.
    ///
    /// `[-tau/4, +tau/4]`
    fn pitch(&self) -> Option<f32> {
        if self.up == Vec3::ZERO {
            None
        } else {
            Some(self.fwd().dot(self.up).clamp(-1.0, 1.0).asin())
        }
    }

    fn set_dir(&mut self, fwd: Vec3) {
        if self.up == Vec3::ZERO {
            self.world_from_view_rot = Quat::from_rotation_arc(-Vec3::Z, fwd);
        } else {
            let pitch = self
                .pitch()
                .unwrap()
                .clamp(-Self::MAX_PITCH, Self::MAX_PITCH);

            let fwd = project_onto(fwd, self.up).normalize(); // Remove pitch
            let right = fwd.cross(self.up).normalize();
            let fwd = Quat::from_axis_angle(right, pitch) * fwd; // Tilt up/down
            let fwd = fwd.normalize(); // Prevent drift

            self.world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, fwd, self.up).inverse());
        }
    }

    pub fn set_up(&mut self, up: Vec3) {
        self.up = up.normalize_or_zero();

        if self.up != Vec3::ZERO {
            self.set_dir(self.fwd()); // this will clamp the rotation
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
            let fwd = Quat::from_axis_angle(self.up, -delta.x) * self.fwd();
            let fwd = fwd.normalize(); // Prevent drift

            let pitch = self.pitch().unwrap() - delta.y;
            let pitch = pitch.clamp(-Self::MAX_PITCH, Self::MAX_PITCH);

            let fwd = project_onto(fwd, self.up).normalize(); // Remove pitch
            let right = fwd.cross(self.up).normalize();
            let fwd = Quat::from_axis_angle(right, pitch) * fwd; // Tilt up/down
            let fwd = fwd.normalize(); // Prevent drift

            self.world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, fwd, self.up).inverse());
        }
    }

    /// Translate based on a certain number of pixel delta.
    pub fn translate(&mut self, delta: egui::Vec2) {
        let delta = delta * self.radius * 0.001; // TODO: take fov and screen size into account?

        let up = self.world_from_view_rot * Vec3::Y;
        let right = self.world_from_view_rot * -Vec3::X; // TODO: why do we need a negation here? O.o

        let translate = delta.x * right + delta.y * up;

        self.center += translate;
    }
}

/// e.g. up is [0,0,1], we return things like [x,y,0]
fn project_onto(v: Vec3, up: Vec3) -> Vec3 {
    v - up * v.dot(up)
}
