use egui::{NumExt as _, Rect};
use glam::{Mat4, Quat, Vec3, vec3};
use macaw::IsoTransform;
use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::blueprint::components::{AngularSpeed, Eye3DKind};
use re_sdk_types::components::{LinearSpeed, Position3D, Vector3D};
use re_ui::ContextExt as _;
use re_view::controls::{
    DRAG_PAN3D_BUTTON, ROLL_MOUSE, ROLL_MOUSE_ALT, ROLL_MOUSE_MODIFIER, ROTATE3D_BUTTON,
    RuntimeModifiers, SPEED_UP_3D_MODIFIER,
};
use re_viewer_context::{ViewContext, ViewerContext};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

use crate::pinhole_wrapper::PinholeWrapper;
use crate::scene_bounding_boxes::SceneBoundingBoxes;

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

    pub fn from_camera(camera: &PinholeWrapper) -> Option<Self> {
        let fov_y = camera.pinhole.fov_y();

        Some(Self {
            world_from_rub_view: camera.world_from_rub_view()?,
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

#[derive(Clone, Debug, PartialEq)]
struct EyeInterpolation {
    elapsed_time: f32,
    start: Eye,
}

impl EyeInterpolation {
    pub fn target_time(start: &Eye, stop: &Eye) -> Option<f32> {
        // Take more time if the rotation is big:
        let angle_difference = start
            .world_from_rub_view
            .rotation()
            .angle_between(stop.world_from_rub_view.rotation());

        // Threshold to avoid doing pointless interpolations that trigger frame requests.
        let distance = start.pos_in_world().distance(stop.pos_in_world());
        if angle_difference < 0.01 && distance < 0.0001 {
            None
        } else {
            Some(egui::remap_clamp(
                angle_difference,
                0.0..=std::f32::consts::PI,
                0.2..=0.7,
            ))
        }
    }
}

/// Some non-persistent state for the eye.
///
/// Note: we use "eye" so we don't confuse this with logged camera.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct EyeState {
    /// Vertical field of view in radians.
    fov_y: Option<f32>,

    velocity: Vec3,

    /// The lasst tracked entity.
    ///
    /// This should not be used to get the current tracked entity, get that
    /// via view properties instead.
    pub last_tracked_entity: Option<EntityPath>,

    interpolation: Option<EyeInterpolation>,

    /// How many radians the camera has spun.
    spin: Option<f64>,

    pub last_eye: Option<Eye>,

    /// The time this was last interacted with in egui time.
    ///
    /// None: Hasn't been interacted with yet.
    pub last_interaction_time: Option<f64>,
    pub last_look_target: Option<Vec3>,
    pub last_orbit_radius: Option<f32>,
    pub last_eye_up: Option<Vec3>,
}

/// Utility struct for handling eye control parameter changes,
/// e.g. via user input or blueprint.
struct EyeController {
    pos: Vec3,
    look_target: Vec3,
    kind: Eye3DKind,
    speed: f64,
    eye_up: Vec3,
    fov_y: Option<f32>,

    did_interact: bool,
}

impl EyeController {
    /// Avoids zentith/nadir singularity.
    const MAX_PITCH: f32 = 0.99 * 0.25 * std::f32::consts::TAU;

    fn get_eye(&self) -> Eye {
        Eye {
            world_from_rub_view: IsoTransform::look_at_rh(
                self.pos,
                if self.pos.distance_squared(self.look_target) < 1e-6 {
                    self.pos + Vec3::Y
                } else {
                    self.look_target
                },
                self.up(),
            )
            .unwrap_or_else(|| IsoTransform::from_translation(self.pos))
            .inverse(),
            fov_y: Some(self.fov_y.unwrap_or(Eye::DEFAULT_FOV_Y)),
        }
    }

    fn from_blueprint(
        ctx: &ViewContext<'_>,
        eye_property: &ViewProperty,
        fov_y: Option<f32>,
    ) -> Result<Self, ViewPropertyQueryError> {
        let kind = eye_property
            .component_or_fallback::<Eye3DKind>(ctx, EyeControls3D::descriptor_kind().component)?;

        let speed = **eye_property.component_or_fallback::<LinearSpeed>(
            ctx,
            EyeControls3D::descriptor_speed().component,
        )?;

        let pos = Vec3::from(eye_property.component_or_fallback::<Position3D>(
            ctx,
            EyeControls3D::descriptor_position().component,
        )?);

        let look_target = Vec3::from(eye_property.component_or_fallback::<Position3D>(
            ctx,
            EyeControls3D::descriptor_look_target().component,
        )?);

        let eye_up = Vec3::from_array(
            eye_property
                .component_or_fallback::<Vector3D>(
                    ctx,
                    EyeControls3D::descriptor_eye_up().component,
                )?
                .0
                .0,
        );

        Ok(Self {
            pos,
            look_target,
            kind,
            speed,
            eye_up,
            did_interact: false,
            fov_y,
        })
    }

    fn copy_from_eye(&mut self, eye: &Eye) {
        self.pos = eye.pos_in_world();
        self.look_target = eye.pos_in_world() + eye.forward_in_world();
        self.eye_up = eye.world_from_rub_view.transform_vector3(Vec3::Y);
        self.fov_y = eye.fov_y;
    }

    /// Saves the subset of eye controls that can change through user input to the blueprint.
    /// Does nothing if no interaction happened.
    fn save_to_blueprint(
        &self,
        ctx: &ViewerContext<'_>,
        eye_property: &ViewProperty,
        old_pos: Vec3,
        old_look_target: Vec3,
        old_eye_up: Vec3,
    ) {
        if !self.did_interact {
            return;
        }

        // If any of these change because of interactions don't use fallback for the other.
        if self.pos != old_pos || self.look_target != old_look_target {
            eye_property.save_blueprint_component(
                ctx,
                &EyeControls3D::descriptor_position(),
                &Position3D::from(self.pos),
            );
            eye_property.save_blueprint_component(
                ctx,
                &EyeControls3D::descriptor_look_target(),
                &Position3D::from(self.look_target),
            );
        }

        if self.eye_up != old_eye_up {
            eye_property.save_blueprint_component(
                ctx,
                &EyeControls3D::descriptor_eye_up(),
                &Vector3D::from(self.eye_up),
            );
        }
    }

    /// Normalized world-direction we are looking at.
    fn fwd(&self) -> Vec3 {
        (self.look_target - self.pos).normalize_or(Vec3::Y)
    }

    /// Normalized up vector, guaranteed to not be parallel to [`Self::fwd`].
    fn up(&self) -> Vec3 {
        let fwd = self.fwd();
        let right = if fwd.dot(Vec3::Z).abs() > 0.9999 {
            Vec3::X
        } else {
            fwd.cross(Vec3::Z)
        };

        let fallback = fwd.cross(right).normalize_or_zero();

        let res = self.eye_up.normalize_or(fallback);

        if res.dot(fwd).abs() > 0.9999 {
            fallback
        } else {
            res
        }
    }

    /// `[-tau/4, +tau/4]`
    fn pitch(&self) -> f32 {
        self.fwd().dot(self.up()).clamp(-1.0, 1.0).asin()
    }

    /// Distance from eye position to look target.
    fn radius(&self) -> f32 {
        self.pos.distance(self.look_target)
    }

    fn rotation(&self) -> Quat {
        Quat::look_to_rh(self.fwd(), self.up()).inverse()
    }

    fn apply_rotation_and_radius(&mut self, rot: Quat, d: f32) {
        let new_fwd = rot * Vec3::NEG_Z * d; // view-coordinates are RUB
        match self.kind {
            Eye3DKind::FirstPerson => {
                self.look_target = self.pos + new_fwd;
            }
            Eye3DKind::Orbital => {
                self.pos = self.look_target - new_fwd;
            }
        }
    }

    /// Rotate based on a certain number of pixel delta.
    pub fn rotate(&mut self, delta: egui::Vec2) {
        let sensitivity = 0.004; // radians-per-point. TODO(emilk): take fov_y and canvas size into account

        let delta = sensitivity * delta;

        let mut rot = self.rotation();
        let radius = self.radius();

        let old_pitch = self.pitch();

        // 2-dof rotation

        // Apply change in heading:
        rot = Quat::from_axis_angle(self.up(), -delta.x) * rot;

        // We need to clamp pitch to avoid nadir/zenith singularity:
        let new_pitch = (old_pitch - delta.y).clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
        let pitch_delta = new_pitch - old_pitch;

        // Apply change in pitch:
        rot *= Quat::from_rotation_x(pitch_delta);

        // Avoid numeric drift:
        rot = rot.normalize();

        self.apply_rotation_and_radius(rot, radius);
    }

    /// Rotate around forward axis
    fn roll(&mut self, rect: &egui::Rect, pointer_pos: egui::Pos2, delta: egui::Vec2) {
        let mut rot = self.rotation();
        let radius = self.radius();
        // steering-wheel model
        let rel = pointer_pos - rect.center();
        let delta_angle = delta.rot90().dot(rel) / rel.length_sq();
        let rot_delta = Quat::from_rotation_z(delta_angle);

        let up_in_view = rot.inverse() * self.eye_up;

        rot *= rot_delta;

        // Permanently change our up-axis, at least until the user resets the view:
        self.eye_up = (rot * up_in_view).normalize_or_zero();

        // Prevent numeric drift:
        rot = rot.normalize();

        self.apply_rotation_and_radius(rot, radius);
    }

    /// Given a delta in view-space, translate the eye.
    fn translate(&mut self, delta_in_view: egui::Vec2) {
        let rot = self.rotation();
        let up = rot * Vec3::Y;
        let right = rot * -Vec3::X; // TODO(emilk): why do we need a negation here? O.o

        let translate = delta_in_view.x * right + delta_in_view.y * up;

        self.pos += translate;
        self.look_target += translate;
    }

    fn handle_drag(&mut self, response: &egui::Response, drag_threshold: f32) {
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
                let pan_speed = 0.001 * self.speed;
                let delta_in_view = pan_speed as f32 * response.drag_delta();

                self.translate(delta_in_view);
            }
        }
    }

    /// Handle zoom/scroll input.
    fn handle_zoom(&mut self, egui_ctx: &egui::Context) {
        let zoom_factor = egui_ctx.input(|input| {
            // egui's default horizontal_scroll_modifier is shift, which is also our speed-up modifier.
            // This means that a user who wants to speed up scroll-to-zoom will generate a horizontal scroll delta.
            // To support that, we have to check and use the horizontal delta if no vertical delta is present (see: #11813).
            let scroll_delta = input.smooth_scroll_delta.x + input.smooth_scroll_delta.y;
            input.zoom_delta() * (scroll_delta / 200.0).exp()
        });

        if zoom_factor == 1.0 {
            return;
        }

        match self.kind {
            Eye3DKind::Orbital => {
                let radius = self.pos.distance(self.look_target);
                let new_radius = radius / zoom_factor;

                // The user may be scrolling to move the camera closer, but are not realizing
                // the radius is now tiny.
                // TODO(emilk): inform the users somehow that scrolling won't help, and that they should use WSAD instead.
                // It might be tempting to start moving the camera here on scroll, but that would is bad for other reasons.

                // Don't let radius go too small or too big because this might cause infinity/nan in some calculations.
                // Max value is chosen with some generous margin of an observed crash due to infinity.
                if f32::MIN_POSITIVE < new_radius && new_radius < 1.0e17 {
                    self.pos = self.look_target - self.fwd() * new_radius;
                    self.did_interact = true;
                }
            }
            Eye3DKind::FirstPerson => {
                // Move along the forward axis when zooming in first person mode.
                let delta = (zoom_factor - 1.0) * self.speed as f32;
                let fwd = self.fwd();
                self.pos += delta * fwd;
                self.look_target = self.pos + fwd;
                self.did_interact = true;
            }
        }
    }

    /// Listen to WSAD and QE to move the eye.
    fn handle_keyboard_navigation(&mut self, eye_state: &mut EyeState, egui_ctx: &egui::Context) {
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

            let rot = self.rotation();

            let world_movement = rot * (self.speed as f32 * local_movement);

            // If input is zero, don't continue moving with velocity. Since we're no longer interacting
            // we don't want to continue writing to blueprint and creating undo points.
            eye_state.velocity = if local_movement == Vec3::ZERO {
                Vec3::ZERO
            } else {
                egui::lerp(
                    eye_state.velocity..=world_movement,
                    egui::emath::exponential_smooth_factor(0.90, 0.2, dt),
                )
            };
            let delta = eye_state.velocity * dt;

            self.pos += delta;
            self.look_target += delta;

            self.did_interact |= local_movement != Vec3::ZERO;
            requires_repaint = local_movement != Vec3::ZERO
                || eye_state.velocity.length() > 0.01 * self.speed as f32;
        });

        if requires_repaint {
            egui_ctx.request_repaint();
        }
    }

    fn handle_input(
        &mut self,
        eye_state: &mut EyeState,
        response: &egui::Response,
        drag_threshold: f32,
    ) {
        // Modify speed based on modifiers:
        let os = response.ctx.os();
        response.ctx.input(|input| {
            if input.modifiers.contains(SPEED_UP_3D_MODIFIER) {
                self.speed *= 10.0;
            }
            if input.modifiers.contains(RuntimeModifiers::slow_down(&os)) {
                self.speed *= 0.1;
            }
        });

        // Dragging even below the [`drag_threshold`] should be considered interaction.
        // Otherwise we flicker in and out of "has interacted" too quickly.
        self.did_interact |= response.drag_delta().length() > 0.0;

        self.handle_drag(response, drag_threshold);

        if response.hovered() {
            self.handle_zoom(&response.ctx);
        }

        if response.has_focus() {
            self.handle_keyboard_navigation(eye_state, &response.ctx);
        } else if response.clicked() || self.did_interact {
            response.request_focus();
        }

        if self.did_interact {
            eye_state.last_interaction_time = Some(response.ctx.time());
        }
    }
}

pub fn find_camera(cameras: &[PinholeWrapper], needle: &EntityPath) -> Option<Eye> {
    let mut found_camera = None;

    for camera in cameras {
        if &camera.ent_path == needle {
            if found_camera.is_some() {
                return None; // More than one camera
            } else {
                found_camera = Some(camera);
            }
        }
    }

    found_camera.and_then(Eye::from_camera)
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t) * (1. - t)
}

impl EyeState {
    /// Sets the eye in an interpolation state, which completes after [`EyeInterpolation::target_time`]
    /// or when the interpolated eye has reached the eye either defined in blueprint or from the eye we're
    /// currently tracking.
    pub fn start_interpolation(&mut self) {
        if let Some(start) = self.last_eye {
            self.interpolation = Some(EyeInterpolation {
                elapsed_time: 0.0,
                start,
            });
        }
    }

    fn stop_interpolation(&mut self) {
        self.interpolation = None;
    }

    /// Gets and updates the current target eye from/to the blueprint.
    fn control_and_sync_with_blueprint(
        &mut self,
        ctx: &ViewContext<'_>,
        eye_property: &ViewProperty,
        response: &egui::Response,
        cameras: &[PinholeWrapper],
        bounding_boxes: &SceneBoundingBoxes,
    ) -> Result<Eye, ViewPropertyQueryError> {
        let mut eye_controller = EyeController::from_blueprint(ctx, eye_property, self.fov_y)?;

        // Save values before mutating the eye to check if they changed later.
        let EyeController {
            pos: old_pos,
            look_target: old_look_target,
            eye_up: old_eye_up,
            ..
        } = eye_controller;

        let mut drag_threshold = 0.0;

        let tracking_entity = eye_property
            .component_or_empty::<re_sdk_types::components::EntityPath>(
                EyeControls3D::descriptor_tracking_entity().component,
            )?
            .and_then(|tracking_entity| {
                if tracking_entity.is_empty() {
                    None
                } else {
                    Some(tracking_entity)
                }
            });

        if let Some(tracking_entity) = &tracking_entity {
            let tracking_entity = EntityPath::from(tracking_entity.as_str());
            if find_camera(cameras, &tracking_entity).is_some() {
                drag_threshold = 0.04;
            }
        }

        // Handle spinning before inputs because some inputs depend on view direction.
        self.handle_spinning(ctx, eye_property, &mut eye_controller)?;

        // We do input before tracking entity, because the input can cause the eye
        // to stop tracking.
        eye_controller.handle_input(self, response, drag_threshold);

        // If we interacted we write to the blueprint so reset spin offset.
        if eye_controller.did_interact {
            self.spin = None;
        }

        eye_controller.save_to_blueprint(
            ctx.viewer_ctx,
            eye_property,
            old_pos,
            old_look_target,
            old_eye_up,
        );

        if let Some(tracked_eye) = self.handle_tracking_entity(
            ctx,
            eye_property,
            cameras,
            bounding_boxes,
            &mut eye_controller,
            old_pos,
            old_look_target,
            old_eye_up,
            tracking_entity.as_ref(),
        ) {
            return Ok(tracked_eye);
        }

        self.last_look_target = Some(eye_controller.look_target);
        self.last_orbit_radius = Some(eye_controller.pos.distance(eye_controller.look_target));
        self.last_eye_up = Some(eye_controller.up());

        Ok(eye_controller.get_eye())
    }

    /// Handles both tracking and clearing tracked entity.
    ///
    /// If we are tracking an entity, this will return the current eye we should use.
    #[expect(clippy::too_many_arguments)]
    fn handle_tracking_entity(
        &mut self,
        ctx: &ViewContext<'_>,
        eye_property: &ViewProperty,
        cameras: &[PinholeWrapper],
        bounding_boxes: &SceneBoundingBoxes,
        eye_controller: &mut EyeController,
        old_pos: Vec3,
        old_look_target: Vec3,
        old_eye_up: Vec3,
        tracking_entity: Option<&re_sdk_types::components::EntityPath>,
    ) -> Option<Eye> {
        if let Some(tracking_entity) = &tracking_entity {
            let tracking_entity = EntityPath::from(tracking_entity.as_str());

            let new_tracking = self.last_tracked_entity.as_ref() != Some(&tracking_entity);
            if new_tracking {
                self.start_interpolation();
                self.last_tracked_entity = Some(tracking_entity.clone());
            }

            // If the position that doesn't move by changing the eye's rotation changed.
            let did_eye_orbit_center_change = match eye_controller.kind {
                Eye3DKind::FirstPerson => eye_controller.pos != old_pos,
                Eye3DKind::Orbital => eye_controller.look_target != old_look_target,
            };

            if let Some(target_eye) = find_camera(cameras, &tracking_entity) {
                if eye_controller.did_interact
                    && (eye_controller.pos != old_pos
                        || eye_controller.look_target != old_look_target)
                {
                    // When we stop tracking, set the blueprint eye state to the tracked view.
                    eye_controller.copy_from_eye(&target_eye);
                    eye_controller.save_to_blueprint(
                        ctx.viewer_ctx,
                        eye_property,
                        old_pos,
                        old_look_target,
                        old_eye_up,
                    );
                    eye_property.clear_blueprint_component(
                        ctx.viewer_ctx,
                        EyeControls3D::descriptor_tracking_entity(),
                    );
                } else {
                    return Some(target_eye);
                }
            } else {
                // Note that we may want to focus on an _instance_ instead in the future:
                // The problem with that is that there may be **many** instances (think point cloud)
                // and they may not be consistent over time.
                // -> we don't know the bounding box of every instance (right now)
                // -> tracking instances over time may not be desired
                //    (this can happen with entities as well, but is less likely).
                //
                // For future reference, it's also worth pointing out that for interactions in the view we
                // already have the 3D position:
                // if let Some(SelectedSpaceContext::ThreeD {
                //     pos: Some(clicked_point),
                //     ..
                // }) = ctx.selection_state().hovered_space_context()

                if let Some(entity_bbox) = bounding_boxes.per_entity.get(&tracking_entity.hash()) {
                    // If we're tracking something new, set the current position & look target to the correct view.
                    if new_tracking {
                        let fwd = eye_controller.fwd();
                        let radius = entity_bbox.centered_bounding_sphere_radius() * 1.5;
                        let radius = if radius < 0.0001 {
                            // Handle zero-sized bounding boxes:
                            (bounding_boxes.current.centered_bounding_sphere_radius() * 1.5)
                                .at_least(0.02)
                        } else {
                            radius
                        };
                        eye_controller.pos = eye_controller.look_target - fwd * radius;
                        // Force write of pos and look target to not use fallbacks for that.
                        eye_property.save_blueprint_component(
                            ctx.viewer_ctx,
                            &EyeControls3D::descriptor_position(),
                            &Position3D::from(eye_controller.pos),
                        );

                        eye_property.save_blueprint_component(
                            ctx.viewer_ctx,
                            &EyeControls3D::descriptor_look_target(),
                            &Position3D::from(eye_controller.look_target),
                        );
                    }

                    let orbit_radius = eye_controller.pos.distance(eye_controller.look_target);

                    let pos = entity_bbox.center();

                    let fwd = eye_controller.fwd();

                    match eye_controller.kind {
                        Eye3DKind::FirstPerson => {
                            eye_controller.pos = pos;
                            eye_controller.look_target = pos + fwd;
                        }
                        Eye3DKind::Orbital => {
                            eye_controller.look_target = pos;
                            eye_controller.pos = pos - fwd * orbit_radius;
                        }
                    }
                }

                self.last_look_target = Some(eye_controller.look_target);
                self.last_eye_up = Some(eye_controller.eye_up);
                self.last_orbit_radius =
                    Some(eye_controller.pos.distance(eye_controller.look_target));

                // When we stop tracking, set the blueprint eye state to the tracked view.
                if eye_controller.did_interact && did_eye_orbit_center_change {
                    eye_controller.save_to_blueprint(
                        ctx.viewer_ctx,
                        eye_property,
                        old_pos,
                        old_look_target,
                        old_eye_up,
                    );

                    eye_property.clear_blueprint_component(
                        ctx.viewer_ctx,
                        EyeControls3D::descriptor_tracking_entity(),
                    );
                } else {
                    return Some(eye_controller.get_eye());
                }
            }
        } else {
            self.last_tracked_entity = None;
        }

        None
    }

    /// Spins the view if `spin_speed` isn't zero.
    fn handle_spinning(
        &mut self,
        ctx: &ViewContext<'_>,
        eye_property: &ViewProperty,
        eye_controller: &mut EyeController,
    ) -> Result<(), ViewPropertyQueryError> {
        let spin_speed = **eye_property.component_or_fallback::<AngularSpeed>(
            ctx,
            EyeControls3D::descriptor_spin_speed().component,
        )?;

        let mut apply_spin = |spin| {
            let quat = Quat::from_axis_angle(eye_controller.up(), spin as f32);

            let fwd = quat * eye_controller.fwd();

            match eye_controller.kind {
                Eye3DKind::FirstPerson => {
                    eye_controller.look_target = eye_controller.pos + fwd;
                }
                Eye3DKind::Orbital => {
                    let d = eye_controller.pos.distance(eye_controller.look_target);
                    eye_controller.pos = eye_controller.look_target - fwd * d;
                }
            }
        };

        if spin_speed != 0.0 {
            let spin = self.spin.get_or_insert_default();

            *spin += spin_speed * ctx.egui_ctx().input(|i| i.stable_dt as f64).at_most(0.1);
            *spin %= std::f64::consts::TAU;

            apply_spin(*spin);

            // Request repaint if we're spinning.
            ctx.egui_ctx().request_repaint();
        }
        // If we just stopped spinning write new position to the blueprint.
        else if let Some(spin) = self.spin.take() {
            apply_spin(spin);
            eye_controller.did_interact = true;
        }

        Ok(())
    }

    pub fn focus_entity(
        &self,
        ctx: &ViewContext<'_>,
        cameras: &[PinholeWrapper],
        bounding_boxes: &SceneBoundingBoxes,
        eye_property: &ViewProperty,
        focused_entity: &EntityPath,
    ) -> Result<(), ViewPropertyQueryError> {
        let mut eye_controller = EyeController::from_blueprint(ctx, eye_property, self.fov_y)?;
        eye_controller.did_interact = true;
        let EyeController {
            pos: old_pos,
            look_target: old_look_target,
            eye_up: old_eye_up,
            ..
        } = eye_controller;
        // Focusing cameras is not something that happens now, since those are always tracked.
        if let Some(target_eye) = find_camera(cameras, focused_entity) {
            eye_controller.copy_from_eye(&target_eye);
        } else if let Some(entity_bbox) = bounding_boxes.per_entity.get(&focused_entity.hash()) {
            let fwd = self
                .last_eye
                .map(|eye| eye.forward_in_world())
                .unwrap_or_else(|| Vec3::splat(f32::sqrt(1.0 / 3.0)));
            let radius = entity_bbox.centered_bounding_sphere_radius() * 1.5;
            let radius = if radius < 0.0001 {
                // Handle zero-sized bounding boxes:
                (bounding_boxes.current.centered_bounding_sphere_radius() * 1.5).at_least(0.02)
            } else {
                radius
            };
            eye_controller.look_target = entity_bbox.center();
            eye_controller.pos = eye_controller.look_target - fwd * radius;
        }

        eye_controller.save_to_blueprint(
            ctx.viewer_ctx,
            eye_property,
            old_pos,
            old_look_target,
            old_eye_up,
        );

        eye_property
            .clear_blueprint_component(ctx.viewer_ctx, EyeControls3D::descriptor_tracking_entity());

        Ok(())
    }

    pub fn update(
        &mut self,
        ctx: &ViewContext<'_>,
        response: &egui::Response,
        pinhole_cameras: &[PinholeWrapper],
        bounding_boxes: &SceneBoundingBoxes,
    ) -> Result<Eye, ViewPropertyQueryError> {
        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.blueprint_db(),
            ctx.blueprint_query(),
            ctx.view_id,
        );

        let target_eye = self.control_and_sync_with_blueprint(
            ctx,
            &eye_property,
            response,
            pinhole_cameras,
            bounding_boxes,
        )?;

        // If we use fallbacks for position and look target, continue to
        // interpolate to the new default eye. This gives much better robustness
        // with scenes that change over time.
        if eye_property
            .component_or_empty::<Position3D>(EyeControls3D::descriptor_position().component)?
            .is_none()
            && eye_property
                .component_or_empty::<Position3D>(
                    EyeControls3D::descriptor_look_target().component,
                )?
                .is_none()
        {
            // Calling [`Self::start_interpolation`] restarts the interpolation state.
            self.start_interpolation();
        }

        let eye = if let Some(interpolation) = &mut self.interpolation
            && let Some(target_time) =
                EyeInterpolation::target_time(&interpolation.start, &target_eye)
        {
            interpolation.elapsed_time += ctx.egui_ctx().input(|i| i.stable_dt).at_most(0.1);

            let t = interpolation.elapsed_time / target_time;
            let t = t.clamp(0.0, 1.0);
            let t = ease_out(t);

            if t < 1.0 {
                // Make sure to repaint if we're interpolating.
                ctx.egui_ctx().request_repaint();

                interpolation.start.lerp(&target_eye, t)
            } else {
                self.stop_interpolation();
                target_eye
            }
        } else {
            self.stop_interpolation();
            target_eye
        };

        self.last_eye = Some(eye);

        Ok(eye)
    }
}
