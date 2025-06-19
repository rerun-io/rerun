use egui::{NumExt as _, Rect, lerp};
use glam::{Mat4, Quat, Vec3, vec3};

use macaw::IsoTransform;

use re_view::controls::{
    DRAG_PAN3D_BUTTON, ROLL_MOUSE, ROLL_MOUSE_ALT, ROLL_MOUSE_MODIFIER, ROTATE3D_BUTTON,
    RuntimeModifiers, SPEED_UP_3D_MODIFIER,
};

use crate::{scene_bounding_boxes::SceneBoundingBoxes, space_camera_3d::SpaceCamera3D};

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

    pub fn from_camera(space_cameras: &SpaceCamera3D) -> Option<Self> {
        let fov_y = space_cameras
            .pinhole
            .as_ref()
            .map_or(Self::DEFAULT_FOV_Y, |pinhole| pinhole.fov_y());

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
            macaw::Ray3::from_origin_dir(self.pos_in_world(), ray_dir.normalize_or_zero())
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

        Self {
            world_from_rub_view: IsoTransform::from_rotation_translation(rotation, translation),
            fov_y,
        }
    }
}

// ----------------------------------------------------------------------------

/// The mode of an [`ViewEye`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum EyeMode {
    FirstPerson,

    #[default]
    Orbital,
}

/// The speed of a [`ViewEye`] can be computed automatically or set manually.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
enum SpeedControl {
    /// [`ViewEye`] speed is computed using heuristics (depending on the mode), see [`fallback_speed_for_mode`]
    Auto,
    /// [`ViewEye`] speed is set to a specific value via the UI (user action), or during interpolation.
    Override(f32),
}

/// An eye (camera) in 3D space, controlled by the user.
///
/// This is either a first person camera or an orbital camera,
/// controlled by [`EyeMode`].
/// We combine these two modes in one struct because they share a lot of state and logic.
///
/// Note: we use "eye" so we don't confuse this with logged camera.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ViewEye {
    /// First person or orbital?
    mode: EyeMode,

    /// Center of orbit, or camera position in first person mode.
    center: Vec3,

    /// Ignored for [`EyeMode::FirstPerson`],
    /// but kept for if/when the user switches to orbital mode.
    orbit_radius: f32,

    /// Rotate to world-space from view-space (RUB).
    world_from_view_rot: Quat,

    /// Vertical field of view in radians.
    fov_y: f32,

    /// The up-axis of the eye itself, in world-space.
    ///
    /// Initially, the up-axis of the eye will be the same as the up-axis of the scene (or +Z if
    /// the scene has no up axis defined).
    /// Rolling the camera (e.g. middle-click) will permanently modify the eye's up axis, until the
    /// next reset.
    ///
    /// A value of `Vec3::ZERO` is valid and will result in 3 degrees of freedom, although we never
    /// use it at the moment.
    eye_up: Vec3,

    /// For controlling the eye with WSAD in a smooth way.
    speed: SpeedControl,
    velocity: Vec3,
}

impl ViewEye {
    /// Avoids zentith/nadir singularity.
    const MAX_PITCH: f32 = 0.99 * 0.25 * std::f32::consts::TAU;

    pub fn new_orbital(
        orbit_center: Vec3,
        orbit_radius: f32,
        world_from_view_rot: Quat,
        eye_up: Vec3,
    ) -> Self {
        Self {
            mode: EyeMode::Orbital,
            center: orbit_center,
            orbit_radius,
            world_from_view_rot,
            fov_y: Eye::DEFAULT_FOV_Y,
            eye_up,
            speed: SpeedControl::Auto,
            velocity: Vec3::ZERO,
        }
    }

    pub fn mode(&self) -> EyeMode {
        self.mode
    }

    pub fn set_mode(&mut self, new_mode: EyeMode) {
        if self.mode != new_mode {
            // Keep the same position:
            match new_mode {
                EyeMode::FirstPerson => self.center = self.position(),
                EyeMode::Orbital => {
                    self.center = self.position() + self.orbit_radius * self.fwd();
                }
            }

            self.mode = new_mode;
        }
    }

    /// If in orbit mode, what are we orbiting around?
    pub fn orbit_center(&self) -> Option<Vec3> {
        match self.mode {
            EyeMode::FirstPerson => None,
            EyeMode::Orbital => Some(self.center),
        }
    }

    /// If in orbit mode, how far from the orbit center are we?
    pub fn orbit_radius(&self) -> Option<f32> {
        match self.mode {
            EyeMode::FirstPerson => None,
            EyeMode::Orbital => Some(self.orbit_radius),
        }
    }

    /// Set what we orbit around, and at what distance.
    ///
    /// If we are not in orbit mode, the state will still be set and used if the user switches to orbit mode.
    pub fn set_orbit_center_and_radius(&mut self, orbit_center: Vec3, orbit_radius: f32) {
        // Temporarily switch to orbital, set the values, and then switch back.
        // This ensures the camera position will be set correctly, even if we
        // were in first-person mode:
        let old_mode = self.mode();
        self.set_mode(EyeMode::Orbital);
        self.center = orbit_center;
        self.orbit_radius = orbit_radius;
        self.set_mode(old_mode);
    }

    /// The world-space position of the eye.
    pub fn position(&self) -> Vec3 {
        match self.mode {
            EyeMode::FirstPerson => self.center,
            EyeMode::Orbital => self.center - self.orbit_radius * self.fwd(),
        }
    }

    /// Compute the actual speed depending on the [`EyeMode`].
    fn fallback_speed_for_mode(&self, bounding_boxes: &SceneBoundingBoxes) -> f32 {
        match self.mode {
            EyeMode::FirstPerson => 0.1 * bounding_boxes.current.size().length(),
            EyeMode::Orbital => self.orbit_radius,
        }
    }

    /// Returns the actual speed (float) of [`ViewEye`].
    pub fn speed(&self, bounding_boxes: &SceneBoundingBoxes) -> f32 {
        match self.speed {
            SpeedControl::Auto => self.fallback_speed_for_mode(bounding_boxes),
            SpeedControl::Override(speed) => speed,
        }
    }

    /// Set the speed to a specific value set by the user via the UI.
    pub fn set_speed(&mut self, new_speed: f32) {
        self.speed = SpeedControl::Override(new_speed);
    }

    /// The local up-axis, if set
    pub fn eye_up(&self) -> Option<Vec3> {
        self.eye_up.try_normalize()
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

    /// Create an [`ViewEye`] from a [`Eye`].
    pub fn copy_from_eye(&mut self, eye: &Eye) {
        match self.mode {
            EyeMode::FirstPerson => {
                self.center = eye.pos_in_world();
            }

            EyeMode::Orbital => {
                // The hard part is finding a good center. Let's try to keep the same, and see how that goes:
                let distance = eye
                    .forward_in_world()
                    .dot(self.center - eye.pos_in_world())
                    .abs();
                self.orbit_radius = distance.at_least(self.orbit_radius / 5.0);
                self.center = eye.pos_in_world() + self.orbit_radius * eye.forward_in_world();
            }
        }
        self.world_from_view_rot = eye.world_from_rub_view.rotation();
        self.fov_y = eye.fov_y.unwrap_or(Eye::DEFAULT_FOV_Y);
        self.velocity = Vec3::ZERO;
        self.speed = SpeedControl::Auto;
        self.eye_up = eye.world_from_rub_view.rotation() * glam::Vec3::Y;
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        if t == 0.0 {
            *self // avoid rounding errors
        } else if t == 1.0 {
            *other // avoid rounding errors
        } else {
            Self {
                mode: other.mode,
                center: self.center.lerp(other.center, t),
                orbit_radius: lerp(self.orbit_radius..=other.orbit_radius, t),
                world_from_view_rot: self.world_from_view_rot.slerp(other.world_from_view_rot, t),
                fov_y: egui::lerp(self.fov_y..=other.fov_y, t),
                // A slerp would technically be nicer for eye_up, but it only really
                // matters if the user starts interacting half-way through the lerp,
                // and even then it's not a big deal.
                eye_up: self.eye_up.lerp(other.eye_up, t).normalize_or_zero(),
                speed: other.speed,
                velocity: self.velocity.lerp(other.velocity, t),
            }
        }
    }

    /// World-direction we are looking at
    fn fwd(&self) -> Vec3 {
        self.world_from_view_rot * -Vec3::Z // view-coordinates are RUB
    }

    /// Only valid if we have an up-vector set.
    ///
    /// `[-tau/4, +tau/4]`
    fn pitch(&self) -> Option<f32> {
        if self.eye_up == Vec3::ZERO {
            None
        } else {
            Some(self.fwd().dot(self.eye_up).clamp(-1.0, 1.0).asin())
        }
    }

    /// Returns `true` if interaction occurred.
    /// I.e. the camera changed via user input.
    pub fn update(
        &mut self,
        response: &egui::Response,
        drag_threshold: f32,
        bounding_boxes: &SceneBoundingBoxes,
    ) -> bool {
        let mut speed = self.speed(bounding_boxes);
        // Modify speed based on modifiers:
        let os = response.ctx.os();
        response.ctx.input(|input| {
            if input.modifiers.contains(SPEED_UP_3D_MODIFIER) {
                speed *= 10.0;
            }
            if input.modifiers.contains(RuntimeModifiers::slow_down(&os)) {
                speed *= 0.1;
            }
        });

        // Dragging even below the [`drag_threshold`] should be considered interaction.
        // Otherwise we flicker in and out of "has interacted" too quickly.
        let mut did_interact = response.drag_delta().length() > 0.0;

        if response.drag_delta().length() > drag_threshold {
            let roll = response.dragged_by(ROLL_MOUSE)
                || (response.dragged_by(ROLL_MOUSE_ALT)
                    && response
                        .ctx
                        .input(|i| i.modifiers.contains(ROLL_MOUSE_MODIFIER)));
            if roll {
                if let Some(pointer_pos) = response.ctx.pointer_latest_pos() {
                    self.roll(&response.rect, pointer_pos, response.drag_delta());
                }
            } else if response.dragged_by(ROTATE3D_BUTTON) {
                self.rotate(response.drag_delta());
            } else if response.dragged_by(DRAG_PAN3D_BUTTON) {
                // The pan speed is selected to make the panning feel natural for orbit mode,
                // but it should probably take FOV and screen size into account
                let pan_speed = 0.001 * speed;
                let delta_in_view = pan_speed * response.drag_delta();

                self.translate(delta_in_view);
            }
        }

        if response.hovered() {
            did_interact |= self.keyboard_navigation(&response.ctx, speed);
        }

        if self.mode == EyeMode::Orbital {
            let (zoom_delta, scroll_delta) = if response.hovered() {
                response
                    .ctx
                    .input(|i| (i.zoom_delta(), i.smooth_scroll_delta.y))
            } else {
                (1.0, 0.0)
            };

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

                did_interact = true;
            }
        }

        did_interact
    }

    /// Listen to WSAD and QE to move the eye.
    ///
    /// Returns `true` if we did anything.
    fn keyboard_navigation(&mut self, egui_ctx: &egui::Context, speed: f32) -> bool {
        let anything_has_focus = egui_ctx.memory(|mem| mem.focused().is_some());
        if anything_has_focus {
            return false; // e.g. we're typing in a TextField
        }

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

            let world_movement = self.world_from_view_rot * (speed * local_movement);

            self.velocity = egui::lerp(
                self.velocity..=world_movement,
                egui::emath::exponential_smooth_factor(0.90, 0.2, dt),
            );
            self.center += self.velocity * dt;

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

        if let Some(old_pitch) = self.pitch() {
            // 2-dof rotation

            // Apply change in heading:
            self.world_from_view_rot =
                Quat::from_axis_angle(self.eye_up, -delta.x) * self.world_from_view_rot;

            // We need to clamp pitch to avoid nadir/zenith singularity:
            let new_pitch = (old_pitch - delta.y).clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
            let pitch_delta = new_pitch - old_pitch;

            // Apply change in pitch:
            self.world_from_view_rot *= Quat::from_rotation_x(pitch_delta);

            // Avoid numeric drift:
            self.world_from_view_rot = self.world_from_view_rot.normalize();
        } else {
            // no up-axis -> no pitch -> 3-dof rotation
            let rot_delta = Quat::from_rotation_y(-delta.x) * Quat::from_rotation_x(-delta.y);
            self.world_from_view_rot *= rot_delta;
        }
    }

    /// Rotate around forward axis
    fn roll(&mut self, rect: &egui::Rect, pointer_pos: egui::Pos2, delta: egui::Vec2) {
        // steering-wheel model
        let rel = pointer_pos - rect.center();
        let delta_angle = delta.rot90().dot(rel) / rel.length_sq();
        let rot_delta = Quat::from_rotation_z(delta_angle);

        let up_in_view = self.world_from_view_rot.inverse() * self.eye_up;

        self.world_from_view_rot *= rot_delta;

        // Permanently change our up-axis, at least until the user resets the view:
        self.eye_up = self.world_from_view_rot * up_in_view;

        // Prevent numeric drift:
        self.world_from_view_rot = self.world_from_view_rot.normalize();
        self.eye_up = self.eye_up.normalize_or_zero();
    }

    /// Given a delta in view-space, translate the eye.
    fn translate(&mut self, delta_in_view: egui::Vec2) {
        let up = self.world_from_view_rot * Vec3::Y;
        let right = self.world_from_view_rot * -Vec3::X; // TODO(emilk): why do we need a negation here? O.o

        let translate = delta_in_view.x * right + delta_in_view.y * up;

        self.center += translate;
    }
}
