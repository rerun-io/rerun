use egui::{lerp, NumExt as _, Rect};
use glam::Affine3A;
use macaw::{vec3, IsoTransform, Mat4, Quat, Vec3};

use re_space_view::controls::{
    RuntimeModifiers, DRAG_PAN3D_BUTTON, ROLL_MOUSE, ROLL_MOUSE_ALT, ROLL_MOUSE_MODIFIER,
    ROTATE3D_BUTTON, SPEED_UP_3D_MODIFIER,
};

use crate::space_camera_3d::SpaceCamera3D;

/// An eye in a 3D view.
///
/// Note: we prefer the word "eye" to not confuse it with logged cameras.
///
/// Our view-space uses RUB (X=Right, Y=Up, Z=Back).
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Eye {
    pub world_from_rub_view: IsoTransform,

    /// If no angle is present, this is an orthographic camera.
    pub fov_y: Option<f32>,
}

impl Eye {
    pub const DEFAULT_FOV_Y: f32 = 55.0_f32 * std::f32::consts::TAU / 360.0;

    pub fn from_camera(space_cameras: &SpaceCamera3D) -> Option<Eye> {
        let fov_y = space_cameras
            .pinhole
            .as_ref()
            .and_then(|pinhole| pinhole.fov_y())
            .unwrap_or(Self::DEFAULT_FOV_Y);

        Some(Self {
            world_from_rub_view: space_cameras.world_from_rub_view()?,
            fov_y: Some(fov_y),
        })
    }

    pub fn near(&self) -> f32 {
        if self.is_perspective() {
            0.01 // TODO(emilk)
        } else {
            -1000.0 // TODO(andreas)
        }
    }

    pub fn far(&self) -> f32 {
        if self.is_perspective() {
            f32::INFINITY
        } else {
            1000.0
        }
    }

    pub fn ui_from_world(&self, space2d_rect: Rect) -> Mat4 {
        let aspect_ratio = space2d_rect.width() / space2d_rect.height();

        let projection = if let Some(fov_y) = self.fov_y {
            Mat4::perspective_infinite_rh(fov_y, aspect_ratio, self.near())
        } else {
            Mat4::orthographic_rh(
                space2d_rect.left(),
                space2d_rect.right(),
                space2d_rect.bottom(),
                space2d_rect.top(),
                self.near(),
                self.far(),
            )
        };

        Mat4::from_translation(vec3(space2d_rect.center().x, space2d_rect.center().y, 0.0))
            * Mat4::from_scale(0.5 * vec3(space2d_rect.width(), -space2d_rect.height(), 1.0))
            * projection
            * self.world_from_rub_view.inverse()
    }

    pub fn is_perspective(&self) -> bool {
        self.fov_y.is_some()
    }

    // pub fn is_orthographic(&self) -> bool {
    //     self.fov_y.is_none()
    // }

    /// Picking ray for a given pointer in the parent space
    /// (i.e. prior to camera transform, "world" space)
    pub fn picking_ray(&self, screen_rect: Rect, pointer: glam::Vec2) -> macaw::Ray3 {
        if let Some(fov_y) = self.fov_y {
            let (w, h) = (screen_rect.width(), screen_rect.height());
            let aspect_ratio = w / h;
            let f = (fov_y * 0.5).tan();
            let px = (2.0 * (pointer.x - screen_rect.left()) / w - 1.0) * f * aspect_ratio;
            let py = (1.0 - 2.0 * (pointer.y - screen_rect.top()) / h) * f;
            let ray_dir = self
                .world_from_rub_view
                .transform_vector3(glam::vec3(px, py, -1.0));
            macaw::Ray3::from_origin_dir(self.pos_in_world(), ray_dir.normalize())
        } else {
            // The ray originates on the camera plane, not from the camera position
            let ray_dir = self.world_from_rub_view.rotation().mul_vec3(glam::Vec3::Z);
            let origin = self.world_from_rub_view.translation()
                + self.world_from_rub_view.rotation().mul_vec3(glam::Vec3::X) * pointer.x
                + self.world_from_rub_view.rotation().mul_vec3(glam::Vec3::Y) * pointer.y
                + ray_dir * self.near();

            macaw::Ray3::from_origin_dir(origin, ray_dir)
        }
    }

    pub fn pos_in_world(&self) -> glam::Vec3 {
        self.world_from_rub_view.translation()
    }

    pub fn forward_in_world(&self) -> glam::Vec3 {
        self.world_from_rub_view.rotation() * -Vec3::Z // because we use RUB
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let translation = self
            .world_from_rub_view
            .translation()
            .lerp(other.world_from_rub_view.translation(), t);
        let rotation = self
            .world_from_rub_view
            .rotation()
            .slerp(other.world_from_rub_view.rotation(), t);

        let fov_y = if t < 0.02 {
            self.fov_y
        } else if t > 0.98 {
            other.fov_y
        } else if self.fov_y.is_none() && other.fov_y.is_none() {
            None
        } else {
            // TODO(andreas): Interpolating between perspective and ortho is untested and likely more involved than this.
            Some(egui::lerp(
                self.fov_y.unwrap_or(0.01)..=other.fov_y.unwrap_or(0.01),
                t,
            ))
        };

        Eye {
            world_from_rub_view: IsoTransform::from_rotation_translation(rotation, translation),
            fov_y,
        }
    }
}

// ----------------------------------------------------------------------------

/// Note: we use "eye" so we don't confuse this with logged camera.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct OrbitEye {
    pub orbit_center: Vec3,
    pub orbit_radius: f32,

    pub world_from_view_rot: Quat,
    pub fov_y: f32,

    /// Zero = no up (3dof rotation)
    pub up: Vec3,

    /// For controlling the eye with WSAD in a smooth way.
    pub velocity: Vec3,
}

impl OrbitEye {
    const MAX_PITCH: f32 = 0.999 * 0.25 * std::f32::consts::TAU;

    pub fn new(orbit_center: Vec3, orbit_radius: f32, world_from_view_rot: Quat, up: Vec3) -> Self {
        OrbitEye {
            orbit_center,
            orbit_radius,
            world_from_view_rot,
            fov_y: Eye::DEFAULT_FOV_Y,
            up,
            velocity: Vec3::ZERO,
        }
    }

    pub fn position(&self) -> Vec3 {
        self.orbit_center + self.world_from_view_rot * vec3(0.0, 0.0, self.orbit_radius)
    }

    pub fn to_eye(self) -> Eye {
        Eye {
            world_from_rub_view: IsoTransform::from_rotation_translation(
                self.world_from_view_rot,
                self.position(),
            ),
            fov_y: Some(self.fov_y),
        }
    }

    /// Create an [`OrbitEye`] from a [`Eye`].
    pub fn copy_from_eye(&mut self, eye: &Eye) {
        // The hard part is finding a good center. Let's try to keep the same, and see how that goes:
        let distance = eye
            .forward_in_world()
            .dot(self.orbit_center - eye.pos_in_world())
            .abs();
        self.orbit_radius = distance.at_least(self.orbit_radius / 5.0);
        self.orbit_center = eye.pos_in_world() + self.orbit_radius * eye.forward_in_world();
        self.world_from_view_rot = eye.world_from_rub_view.rotation();
        self.fov_y = eye.fov_y.unwrap_or(Eye::DEFAULT_FOV_Y);
        self.velocity = Vec3::ZERO;
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        if t == 0.0 {
            *self // avoid rounding errors
        } else if t == 1.0 {
            *other // avoid rounding errors
        } else {
            Self {
                orbit_center: self.orbit_center.lerp(other.orbit_center, t),
                orbit_radius: lerp(self.orbit_radius..=other.orbit_radius, t),
                world_from_view_rot: self.world_from_view_rot.slerp(other.world_from_view_rot, t),
                fov_y: egui::lerp(self.fov_y..=other.fov_y, t),
                up: self.up.lerp(other.up, t).normalize_or_zero(),
                velocity: self.velocity.lerp(other.velocity, t),
            }
        }
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

    fn set_fwd(&mut self, fwd: Vec3) {
        if let Some(pitch) = self.pitch() {
            let pitch = pitch.clamp(-Self::MAX_PITCH, Self::MAX_PITCH);

            let fwd = project_onto(fwd, self.up).normalize(); // Remove pitch
            let right = fwd.cross(self.up).normalize();
            let fwd = Quat::from_axis_angle(right, pitch) * fwd; // Tilt up/down
            let fwd = fwd.normalize(); // Prevent drift

            let world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, fwd, self.up).inverse());

            if world_from_view_rot.is_finite() {
                self.world_from_view_rot = world_from_view_rot;
            }
        } else {
            self.world_from_view_rot = Quat::from_rotation_arc(-Vec3::Z, fwd);
        }
    }

    #[allow(unused)]
    pub fn set_up(&mut self, up: Vec3) {
        self.up = up.normalize_or_zero();

        if self.up != Vec3::ZERO {
            self.set_fwd(self.fwd()); // this will clamp the rotation
        }
    }

    /// Returns `true` if interaction occurred.
    /// I.e. the camera changed via user input.
    pub fn update(&mut self, response: &egui::Response, drag_threshold: f32) -> bool {
        // Dragging even below the [`drag_threshold`] should be considered interaction.
        // Otherwise we flicker in and out of "has interacted" too quickly.
        let mut did_interact = response.drag_delta().length() > 0.0;

        if response.drag_delta().length() > drag_threshold {
            if response.dragged_by(ROLL_MOUSE)
                || (response.dragged_by(ROLL_MOUSE_ALT)
                    && response
                        .ctx
                        .input(|i| i.modifiers.contains(ROLL_MOUSE_MODIFIER)))
            {
                if let Some(pointer_pos) = response.ctx.pointer_latest_pos() {
                    self.roll(&response.rect, pointer_pos, response.drag_delta());
                }
            } else if response.dragged_by(ROTATE3D_BUTTON) {
                self.rotate(response.drag_delta());
            } else if response.dragged_by(DRAG_PAN3D_BUTTON) {
                self.translate(response.drag_delta());
            }
        }

        let (zoom_delta, scroll_delta) = if response.hovered() {
            did_interact |= self.keyboard_navigation(&response.ctx);
            response
                .ctx
                .input(|i| (i.zoom_delta(), i.smooth_scroll_delta.y))
        } else {
            (1.0, 0.0)
        };
        if zoom_delta != 1.0 || scroll_delta.abs() > 0.1 {
            did_interact = true;
        }

        let zoom_factor = zoom_delta * (scroll_delta / 200.0).exp();
        if zoom_factor != 1.0 {
            let new_radius = self.orbit_radius / zoom_factor;

            // The user may be scrolling to move the camera closer, but are not realizing
            // the radius is now tiny.
            // TODO(emilk): inform the users somehow that scrolling won't help, and that they should use WSAD instead.
            // It might be tempting to start moving the camera here on scroll, but that would is bad for other reasons.

            // Don't let radius go too small or too big because this might cause infinity/nan in some calculations.
            // Max value is chosen with some generous margin of an observed crash due to infinity.
            if f32::MIN_POSITIVE < new_radius && new_radius < 1.0e17 {
                self.orbit_radius = new_radius;
            }
        }

        did_interact
    }

    /// Listen to WSAD and QE to move the eye.
    ///
    /// Returns `true` if we did anything.
    fn keyboard_navigation(&mut self, egui_ctx: &egui::Context) -> bool {
        let anything_has_focus = egui_ctx.memory(|mem| mem.focus().is_some());
        if anything_has_focus {
            return false; // e.g. we're typing in a TextField
        }

        let os = egui_ctx.os();

        let mut did_interact = false;
        let mut requires_repaint = false;

        egui_ctx.input(|input| {
            let dt = input.stable_dt.at_most(0.1);

            // X=right, Y=up, Z=back
            let mut local_movement = Vec3::ZERO;
            local_movement.z -= input.key_down(egui::Key::W) as i32 as f32;
            local_movement.z += input.key_down(egui::Key::S) as i32 as f32;
            local_movement.x -= input.key_down(egui::Key::A) as i32 as f32;
            local_movement.x += input.key_down(egui::Key::D) as i32 as f32;
            local_movement.y -= input.key_down(egui::Key::Q) as i32 as f32;
            local_movement.y += input.key_down(egui::Key::E) as i32 as f32;
            local_movement = local_movement.normalize_or_zero();

            let speed = self.orbit_radius
                * (if input.modifiers.contains(SPEED_UP_3D_MODIFIER) {
                    10.0
                } else {
                    1.0
                })
                * (if input.modifiers.contains(RuntimeModifiers::slow_down(&os)) {
                    0.1
                } else {
                    1.0
                });
            let world_movement = self.world_from_view_rot * (speed * local_movement);

            self.velocity = egui::lerp(
                self.velocity..=world_movement,
                egui::emath::exponential_smooth_factor(0.90, 0.2, dt),
            );
            self.orbit_center += self.velocity * dt;

            did_interact = local_movement != Vec3::ZERO;
            requires_repaint =
                local_movement != Vec3::ZERO || self.velocity.length() > 0.01 * speed;
        });

        if requires_repaint {
            egui_ctx.request_repaint();
        }

        did_interact
    }

    /// Rotate based on a certain number of pixel delta.
    pub fn rotate(&mut self, delta: egui::Vec2) {
        let sensitivity = 0.004; // radians-per-point. TODO(emilk): take fov_y and canvas size into account
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

            let new_world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, fwd, self.up).inverse());

            if new_world_from_view_rot.is_finite() {
                self.world_from_view_rot = new_world_from_view_rot;
            } else {
                re_log::debug_once!("Failed to rotate camera: got non-finites");
            }
        }
    }

    /// Rotate around forward axis
    fn roll(&mut self, rect: &egui::Rect, pointer_pos: egui::Pos2, delta: egui::Vec2) {
        // steering-wheel model
        let rel = pointer_pos - rect.center();
        let delta_angle = delta.rot90().dot(rel) / rel.length_sq();
        let rot_delta = Quat::from_rotation_z(delta_angle);
        self.world_from_view_rot *= rot_delta;

        self.up = Vec3::ZERO; // forget about this until user resets the eye
    }

    /// Translate based on a certain number of pixel delta.
    fn translate(&mut self, delta: egui::Vec2) {
        let delta = delta * self.orbit_radius * 0.001; // TODO(emilk): take fov and screen size into account?

        let up = self.world_from_view_rot * Vec3::Y;
        let right = self.world_from_view_rot * -Vec3::X; // TODO(emilk): why do we need a negation here? O.o

        let translate = delta.x * right + delta.y * up;

        self.orbit_center += translate;
    }
}

/// e.g. up is `[0,0,1]`, we return things like `[x,y,0]`
fn project_onto(v: Vec3, up: Vec3) -> Vec3 {
    v - up * v.dot(up)
}
