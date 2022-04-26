use egui::{Color32, Rect};
use glam::Affine3A;
use log_types::{Data, LogId, LogMsg, ObjectPath};
use macaw::{vec3, Quat, Vec3};

use crate::ViewerContext;
use crate::{log_db::SpaceSummary, LogDb};

mod camera;
mod mesh_cache;
mod rendering;
mod scene;

use camera::*;
use mesh_cache::*;
use rendering::*;
use scene::*;

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct State3D {
    camera: Option<OrbitCamera>,
}

fn show_settings_ui(
    ui: &mut egui::Ui,
    state_3d: &mut State3D,
    space: &ObjectPath,
    space_summary: &SpaceSummary,
    space_specs: &SpaceSpecs,
) {
    ui.horizontal(|ui| {
        {
            let up = space_specs.up.normalize_or_zero();

            let up_response = if up == Vec3::X {
                ui.label("Up: +X")
            } else if up == -Vec3::X {
                ui.label("Up: -X")
            } else if up == Vec3::Y {
                ui.label("Up: +Y")
            } else if up == -Vec3::Y {
                ui.label("Up: -Y")
            } else if up == Vec3::Z {
                ui.label("Up: +Z")
            } else if up == -Vec3::Z {
                ui.label("Up: -Z")
            } else if up != Vec3::ZERO {
                ui.label(format!("Up: [{:.3} {:.3} {:.3}]", up.x, up.y, up.z))
            } else {
                ui.label("Up: unspecified")
            };
            up_response.on_hover_ui(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Set by logging to ");
                    ui.code(format!("{space}/up"));
                    ui.label(".");
                });
            });
        }

        if ui.button("Reset camera").clicked() {
            state_3d.camera = Some(default_camera(space_summary, space_specs));
        }

        ui.colored_label(ui.visuals().widgets.inactive.text_color(), "Help!")
            .on_hover_text(
                "Drag to rotate.\nDrag with secondary mouse button to pan.\nScroll to zoom.",
            );
    });
}

#[derive(Default)]
struct SpaceSpecs {
    /// ZERO = unset
    up: glam::Vec3,
}

impl SpaceSpecs {
    fn from_messages(space: &ObjectPath, messages: &[&LogMsg]) -> Self {
        let mut slf = Self::default();

        let up_path = space / "up";

        for msg in messages {
            if msg.object_path == up_path {
                if let Data::Vec3(vec3) = msg.data {
                    slf.up = Vec3::from(vec3).normalize_or_zero();
                } else {
                    tracing::warn!("Expected {} to be a Vec3; got: {:?}", up_path, msg.data);
                }
            }
        }
        slf
    }
}

pub(crate) fn combined_view_3d(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    state_3d: &mut State3D,
    space: &ObjectPath,
    space_summary: &SpaceSummary,
    messages: &[&LogMsg],
) {
    crate::profile_function!();

    let space_specs = SpaceSpecs::from_messages(space, messages);

    // TODO: show settings on top of 3D view.
    // Requires some egui work to handle interaction of overlapping widgets.
    show_settings_ui(ui, state_3d, space, space_summary, &space_specs);

    let frame = egui::Frame::canvas(ui.style()).inner_margin(2.0);
    let (outer_rect, response) =
        ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    // ---------------------------------

    let camera = state_3d
        .camera
        .get_or_insert_with(|| default_camera(space_summary, &space_specs));

    camera.set_up(space_specs.up);

    if response.dragged_by(egui::PointerButton::Primary) {
        camera.rotate(response.drag_delta());
    } else if response.dragged_by(egui::PointerButton::Secondary) {
        camera.translate(response.drag_delta());
    }

    // ---------------------------------

    // TODO: focus
    // if response.clicked() || response.dragged() {
    //     ui.ctx().memory().request_focus(response.id);
    // } else if response.clicked_elsewhere() {
    //     ui.ctx().memory().surrender_focus(response.id);
    // }
    // if ui.ctx().memory().has_focus(response.id) {
    //     frame.stroke = ui.visuals().selection.stroke; // TODO: something less subtle
    //     frame.stroke.width *= 2.0; // hack to make it less subtle
    // }
    // if ui.ctx().memory().has_focus(response.id) {
    //     // TODO: WASD movement
    // }

    if response.hovered() {
        // let factor = ui.input().zoom_delta();
        let factor = (ui.input().scroll_delta.y / 200.0).exp();
        camera.radius /= factor;
    }

    ui.painter().add(frame.paint(outer_rect));

    let inner_rect = outer_rect.shrink2(frame.inner_margin.sum() + frame.outer_margin.sum());

    let camera = camera.to_camera();

    let hovered_id = picking(ui, &inner_rect, space, messages, &camera);
    if let Some(hovered_id) = hovered_id {
        if response.clicked() {
            context.selection = crate::Selection::LogId(hovered_id);
        }

        if let Some(msg) = log_db.get_msg(&hovered_id) {
            egui::containers::popup::show_tooltip_at_pointer(
                ui.ctx(),
                egui::Id::new("3d_tooltip"),
                |ui| {
                    crate::view2d::on_hover_ui(context, ui, msg);
                },
            );
        }
    }

    let mut scene = Scene::default();
    for msg in messages {
        if msg.space.as_ref() == Some(space) {
            let is_hovered = Some(msg.id) == hovered_id;

            // TODO: selection color
            let color = if is_hovered {
                Color32::WHITE
            } else {
                context.object_color(log_db, msg)
            };

            scene.add_msg(
                space_summary,
                inner_rect.size(),
                &camera,
                is_hovered,
                color,
                msg,
            );
        }
    }

    let callback = egui::PaintCallback {
        rect: inner_rect,
        callback: std::sync::Arc::new(move |info, render_ctx| {
            if let Some(painter) = render_ctx.downcast_ref::<egui_glow::Painter>() {
                with_three_d_context(painter.gl(), |rendering| {
                    paint_with_three_d(rendering, &camera, info, &scene).unwrap();
                });
            } else {
                eprintln!("Can't do custom painting because we are not using a glow context");
            }
        }),
    };
    ui.painter().add(callback);
}

fn picking(
    ui: &egui::Ui,
    rect: &Rect,
    space: &ObjectPath,
    messages: &[&LogMsg],
    camera: &Camera,
) -> Option<LogId> {
    crate::profile_function!();

    let pointer_pos = ui.ctx().pointer_hover_pos()?;

    let screen_from_world = camera.screen_from_world(rect);

    let mut closest_dist_sq = 5.0 * 5.0; // TODO: interaction radius from egui
    let mut closest_id = None;

    for msg in messages {
        if msg.space.as_ref() == Some(space) {
            match &msg.data {
                Data::Pos3([x, y, z]) => {
                    let screen_pos = screen_from_world.project_point3(vec3(*x, *y, *z));
                    if screen_pos.z < 0.0 {
                        continue;
                    }
                    let screen_pos = egui::pos2(screen_pos.x, screen_pos.y);

                    let dist_sq = screen_pos.distance_sq(pointer_pos);
                    if dist_sq < closest_dist_sq {
                        closest_dist_sq = dist_sq;
                        closest_id = Some(msg.id);
                    }

                    if false {
                        // good for sanity checking the projection matrix
                        ui.ctx()
                            .debug_painter()
                            .circle_filled(screen_pos, 3.0, egui::Color32::RED);
                    }
                }
                Data::Vec3(_)
                | Data::Box3(_)
                | Data::Path3D(_)
                | Data::LineSegments3D(_)
                | Data::Mesh3D(_)
                | Data::Camera(_) => {
                    // TODO: more picking
                }
                _ => {
                    debug_assert!(!msg.data.is_3d());
                }
            }
        }
    }

    closest_id
}

fn default_camera(space_summary: &SpaceSummary, space_spects: &SpaceSpecs) -> OrbitCamera {
    let bbox = space_summary.bbox3d;

    let mut center = bbox.center();
    if !center.is_finite() {
        center = Vec3::ZERO;
    }

    let mut radius = 3.0 * bbox.half_size().length();
    if !radius.is_finite() || radius == 0.0 {
        radius = 1.0;
    }

    let cam_dir = vec3(1.0, 1.0, 0.5).normalize();
    let camera_pos = center + radius * cam_dir;

    let look_up = if space_spects.up == Vec3::ZERO {
        Vec3::Z
    } else {
        space_spects.up
    };

    OrbitCamera {
        center,
        radius,
        world_from_view_rot: Quat::from_affine3(
            &Affine3A::look_at_rh(camera_pos, center, look_up).inverse(),
        ),
        fov_y: 50.0_f32.to_radians(), // TODO: base on viewport size?
        up: space_spects.up,
    }
}

/// We get a [`glow::Context`] from `eframe`, but we want a [`three_d::Context`].
///
/// Sadly we can't just create and store a [`three_d::Context`] in the app and pass it
/// to the [`egui::PaintCallback`] because [`three_d::Context`] isn't `Send+Sync`, which
/// [`egui::PaintCallback`] is.
fn with_three_d_context<R>(
    gl: &std::rc::Rc<glow::Context>,
    f: impl FnOnce(&mut RenderingContext) -> R,
) -> R {
    use std::cell::RefCell;
    thread_local! {
        static THREE_D: RefCell<Option<RenderingContext>> = RefCell::new(None);
    }

    #[allow(unsafe_code)]
    unsafe {
        use glow::HasContext as _;
        gl.enable(glow::DEPTH_TEST);
        if !cfg!(target_arch = "wasm32") {
            gl.disable(glow::FRAMEBUFFER_SRGB);
        }
        gl.clear(glow::DEPTH_BUFFER_BIT);
    }

    THREE_D.with(|three_d| {
        let mut three_d = three_d.borrow_mut();
        let three_d = three_d.get_or_insert_with(|| RenderingContext::new(gl).unwrap());
        f(three_d)
    })
}
