//! Easter egg: typing ↑ ↑ ↓ ↓ ← → ← → B A turns the Rerun logo into an orbiting, depth-colored point cloud.

use std::sync::{Arc, OnceLock};

use egui::emath::easing::bounce_out;
use egui::emath::remap_clamp;
use egui::epaint::RectShape;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Key, LayerId, Order, Painter, Pos2, Rect, Vec2};
use glam::{Quat, Vec3};
use rand::{Rng as _, SeedableRng as _};

const SEQUENCE: [Key; 10] = [
    Key::ArrowUp,
    Key::ArrowUp,
    Key::ArrowDown,
    Key::ArrowDown,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::B,
    Key::A,
];

const DURATION_SEC: f32 = 6.5;
const FADE_OUT_SEC: f32 = 1.0;
const ICON_LANDING_SEC: f32 = 0.7;
const DISSOLVE_START_SEC: f32 = 1.0;
const DISSOLVE_END_SEC: f32 = 1.5;
const EXTRUDE_END_SEC: f32 = 2.2;
const BURST_START_SEC: f32 = 5.0;

const NUM_POINTS: usize = 3000;

/// Half the extrusion depth of the point cloud, in icon widths.
const HALF_DEPTH: f32 = 0.09;

/// Distance from the orbit camera to the orbit center, in icon widths.
const CAMERA_DISTANCE: f32 = 3.0;

const CAPTION: &str = r#"rr.log("cheats/konami", rr.Points3D(positions, colors=colors))"#;

#[derive(Clone, Default)]
struct State {
    recent_keys: Vec<Key>,
    triggered_at: Option<f64>,
}

fn state_id() -> egui::Id {
    egui::Id::unique("rerun_konami_code")
}

/// Must be called before any `on_begin_pass` hook that consumes arrow keys, or the sequence is never seen.
pub fn install(egui_ctx: &egui::Context) {
    egui_ctx.on_begin_pass("rerun-konami-detect", Arc::new(|ui| detect(ui)));
    egui_ctx.on_end_pass("rerun-konami-paint", Arc::new(|ui| paint(ui)));
}

fn detect(ctx: &egui::Context) {
    if ctx.current_pass_index() != 0 || ctx.text_edit_focused() {
        return;
    }

    let (pressed, now) = ctx.input(|i| {
        let pressed: Vec<Key> = i
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } => Some(*key),
                _ => None,
            })
            .collect();
        (pressed, i.time)
    });
    if pressed.is_empty() {
        return;
    }

    ctx.data_mut(|data| {
        let state = data.get_temp_mut_or_default::<State>(state_id());
        for key in pressed {
            if push_key(&mut state.recent_keys, key) {
                state.triggered_at = Some(now);
            }
        }
    });
}

/// Returns `true` when `key` completes the sequence.
fn push_key(recent_keys: &mut Vec<Key>, key: Key) -> bool {
    recent_keys.push(key);
    if recent_keys.len() > SEQUENCE.len() {
        recent_keys.remove(0);
    }
    if recent_keys.as_slice() == SEQUENCE {
        recent_keys.clear();
        true
    } else {
        false
    }
}

fn paint(ctx: &egui::Context) {
    let Some(triggered_at) = ctx.data(|data| data.get_temp::<State>(state_id())?.triggered_at)
    else {
        return;
    };

    let t = (ctx.input(|i| i.time) - triggered_at) as f32;
    if DURATION_SEC < t {
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<State>(state_id())
                .triggered_at = None;
        });
        return;
    }
    ctx.request_repaint();

    let painter = ctx.layer_painter(LayerId::new(Order::Debug, state_id()));
    let screen = ctx.content_rect();
    let opacity = remap_clamp(t, (DURATION_SEC - FADE_OUT_SEC)..=DURATION_SEC, 1.0..=0.0);

    let icon_size = (0.4 * screen.height()).clamp(96.0, 320.0);
    let icon_center = screen.center() - Vec2::new(0.0, 0.08 * screen.height());

    painter.rect_filled(screen, 0.0, Color32::BLACK.gamma_multiply(0.6 * opacity));

    let cloud = logo_cloud();
    let extrude = smoothstep(DISSOLVE_START_SEC, EXTRUDE_END_SEC, t);
    let camera = OrbitCamera::at(
        t,
        icon_center + cloud.centroid * icon_size,
        icon_size * (1.0 + extrude),
    );

    paint_grid(&painter, &camera, 0.4 * extrude * opacity);
    paint_icon(&painter, icon_center, icon_size, t);
    paint_points(&painter, &camera, cloud, t, extrude, opacity);

    let caption_font = FontId::new(
        (screen.width() / 60.0).clamp(12.0, 22.0),
        FontFamily::Monospace,
    );
    let caption_center = icon_center + Vec2::new(0.0, 0.75 * icon_size);
    paint_caption(
        &painter,
        &caption_font,
        caption_center,
        t - DISSOLVE_START_SEC,
        opacity,
    );
    paint_bouncing_text(
        &painter,
        "↑ ↑ ↓ ↓ ← → ← → B A",
        &FontId::new(1.1 * caption_font.size, FontFamily::Proportional),
        caption_center + Vec2::new(0.0, 2.2 * caption_font.size),
        t - EXTRUDE_END_SEC - 0.6,
        opacity,
    );
}

/// The Rerun app icon, with the same rounded corners, bouncing in and then fading away as its points take over.
fn paint_icon(painter: &Painter, center: Pos2, size: f32, t: f32) {
    let alpha = 1.0 - smoothstep(DISSOLVE_START_SEC, DISSOLVE_END_SEC, t);
    if alpha <= 0.0 {
        return;
    }

    let logo = re_ui::icons::RERUN_LOGO;
    let ctx = painter.ctx();
    ctx.include_bytes(logo.uri(), logo.image_bytes());
    let Ok(egui::load::TexturePoll::Ready { texture }) = ctx.try_load_texture(
        logo.uri(),
        egui::TextureOptions::LINEAR,
        egui::SizeHint::default(),
    ) else {
        return;
    };

    let drop = 1.0 - bounce_out((t / ICON_LANDING_SEC).min(1.0));
    let center = center - Vec2::new(0.0, 2.5 * size * drop);
    let rect = Rect::from_center_size(center, Vec2::splat(size));

    // macOS app icons round their corners at ~22.5% of the icon size.
    let corner_radius = (0.225 * size).round().min(255.0) as u8;

    painter.add(
        RectShape::filled(rect, corner_radius, Color32::WHITE.gamma_multiply(alpha)).with_texture(
            texture.id,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        ),
    );
}

/// Orbits around the cloud like dragging the camera in a 3D view.
struct OrbitCamera {
    rotation: Quat,
    center: Pos2,
    pixels_per_unit: f32,
}

impl OrbitCamera {
    fn at(t: f32, center: Pos2, pixels_per_unit: f32) -> Self {
        let orbit_t = (t - DISSOLVE_START_SEC).max(0.0);
        let ramp = smoothstep(DISSOLVE_START_SEC, EXTRUDE_END_SEC, t);
        let yaw = ramp * 0.8 * (1.1 * orbit_t).sin();
        let pitch = ramp * (0.3 + 0.08 * (0.7 * orbit_t).sin());
        Self {
            rotation: Quat::from_rotation_x(pitch) * Quat::from_rotation_y(yaw),
            center,
            pixels_per_unit,
        }
    }

    /// Returns the screen position, view depth, and perspective scale.
    fn project(&self, point: Vec3) -> (Pos2, f32, f32) {
        let view = self.rotation * point;
        let perspective = CAMERA_DISTANCE / (CAMERA_DISTANCE + view.z);
        let pos = self.center + Vec2::new(view.x, view.y) * (self.pixels_per_unit * perspective);
        (pos, view.z, perspective)
    }
}

/// A floor grid under the cloud that fades out with distance, like the one in the 3D view.
fn paint_grid(painter: &Painter, camera: &OrbitCamera, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }

    let floor_y = 0.22;
    let half_extent = 0.8;
    let num_cells = 16;
    let num_segments = 16;

    let grid_point = |along: f32, across: f32, flip: bool| {
        if flip {
            Vec3::new(across, floor_y, along)
        } else {
            Vec3::new(along, floor_y, across)
        }
    };
    let along = |k: usize| half_extent * (2.0 * k as f32 / num_segments as f32 - 1.0);

    for flip in [false, true] {
        for cell in 0..=num_cells {
            let across = half_extent * (2.0 * cell as f32 / num_cells as f32 - 1.0);
            for segment in 0..num_segments {
                let (a, b) = (along(segment), along(segment + 1));
                let distance = Vec2::new(0.5 * (a + b), across).length() / half_extent;
                let fade = (1.0 - distance).max(0.0).powi(2);
                if fade <= 0.0 {
                    continue;
                }
                let stroke = egui::Stroke::new(1.0, Color32::WHITE.gamma_multiply(alpha * fade));
                painter.line_segment(
                    [
                        camera.project(grid_point(a, across, flip)).0,
                        camera.project(grid_point(b, across, flip)).0,
                    ],
                    stroke,
                );
            }
        }
    }
}

fn paint_points(
    painter: &Painter,
    camera: &OrbitCamera,
    cloud: &LogoCloud,
    t: f32,
    extrude: f32,
    opacity: f32,
) {
    let alpha = smoothstep(DISSOLVE_START_SEC, DISSOLVE_END_SEC, t) * opacity;
    if alpha <= 0.0 {
        return;
    }

    let burst_t = (t - BURST_START_SEC).max(0.0);
    let gravity = Vec3::new(0.0, 2.5, 0.0);

    let mut projected: Vec<_> = cloud
        .points
        .iter()
        .map(|point| {
            let mut pos = point.position;
            pos.z *= HALF_DEPTH * extrude;

            // Colored by depth at the moment of the burst, so points keep their color as they fly apart.
            let (_, depth, _) = camera.project(pos);
            let depth_t = (0.5 - depth / (3.0 * HALF_DEPTH + 0.3)).clamp(0.0, 1.0);
            let color = Color32::WHITE
                .lerp_to_gamma(turbo(depth_t, 1.0), extrude)
                .gamma_multiply(alpha);

            if 0.0 < burst_t {
                let direction = (pos + 0.3 * point.burst_jitter).normalize_or_zero();
                let velocity = direction * point.burst_speed - Vec3::new(0.0, 0.6, 0.0);
                pos += velocity * burst_t + 0.5 * gravity * burst_t * burst_t;
            }

            let (screen_pos, depth, perspective) = camera.project(pos);
            let radius = (0.004 * camera.pixels_per_unit * perspective).max(1.0);
            (depth, screen_pos, radius, color)
        })
        .collect();

    projected.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, pos, radius, color) in projected {
        painter.circle_filled(pos, radius, color);
    }
}

/// Types out the logging call, as if the points were just logged from the SDK.
fn paint_caption(painter: &Painter, font_id: &FontId, center: Pos2, t: f32, opacity: f32) {
    if t < 0.0 {
        return;
    }

    let num_visible = ((40.0 * t) as usize).min(CAPTION.chars().count());
    let code_color = Color32::WHITE.gamma_multiply(0.85 * opacity);
    let string_color = turbo(0.75, opacity);

    let mut job = LayoutJob::default();
    let mut in_string = false;
    for c in CAPTION.chars().take(num_visible) {
        let is_quote = c == '"';
        if is_quote && !in_string {
            in_string = true;
        } else if is_quote {
            in_string = false;
            job.append("\"", 0.0, TextFormat::simple(font_id.clone(), string_color));
            continue;
        }
        let color = if in_string { string_color } else { code_color };
        job.append(
            &c.to_string(),
            0.0,
            TextFormat::simple(font_id.clone(), color),
        );
    }

    let full_width = painter
        .layout_no_wrap(CAPTION.to_owned(), font_id.clone(), code_color)
        .size()
        .x;
    let galley = painter.layout_job(job);
    let pos = Pos2::new(center.x - 0.5 * full_width, center.y - 0.5 * font_id.size);
    let cursor_x = pos.x + galley.size().x;
    painter.galley(pos, galley, code_color);

    if (2.0 * t).fract() < 0.5 {
        let cursor = Rect::from_min_size(
            Pos2::new(cursor_x + 1.0, pos.y),
            Vec2::new(0.55 * font_id.size, 1.2 * font_id.size),
        );
        painter.rect_filled(cursor, 0.0, code_color);
    }
}

struct LogoPoint {
    /// Relative to [`LogoCloud::centroid`], in icon widths; `z` is in `-1..=1`, before extrusion.
    position: Vec3,

    /// Sampled once so burst trajectories remain stable between frames.
    burst_jitter: Vec3,
    burst_speed: f32,
}

struct LogoCloud {
    points: Vec<LogoPoint>,

    /// Center of the "re" letters, relative to the icon center, in icon widths.
    centroid: Vec2,
}

/// Samples the white "re" letters of the app icon, so the cloud lines up with the icon it replaces.
fn logo_cloud() -> &'static LogoCloud {
    static CLOUD: OnceLock<LogoCloud> = OnceLock::new();
    CLOUD.get_or_init(|| {
        let letter_pixels: Vec<Vec2> = image::load_from_memory_with_format(
            re_ui::icons::RERUN_LOGO.image_bytes(),
            image::ImageFormat::Png,
        )
        .map(|image| {
            let image = image.to_rgba8();
            let size = Vec2::new(image.width() as f32, image.height() as f32);
            image
                .enumerate_pixels()
                .filter(|(_, _, pixel)| is_letter_pixel(pixel.0))
                .map(|(x, y, _)| Vec2::new(x as f32, y as f32) / size - Vec2::splat(0.5))
                .collect()
        })
        .unwrap_or_default();

        if letter_pixels.is_empty() {
            return LogoCloud {
                points: Vec::new(),
                centroid: Vec2::ZERO,
            };
        }

        let centroid =
            letter_pixels.iter().fold(Vec2::ZERO, |sum, &p| sum + p) / letter_pixels.len() as f32;
        let pixel_size = 1.0 / 256.0;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        let points = (0..NUM_POINTS)
            .map(|_| {
                let index = rng.random_range(0..letter_pixels.len());
                let p = letter_pixels[index] - centroid
                    + pixel_size * Vec2::new(rng.random(), rng.random());
                LogoPoint {
                    position: Vec3::new(p.x, p.y, rng.random_range(-1.0..1.0)),
                    burst_jitter: Vec3::new(
                        rng.random_range(-0.5..0.5),
                        rng.random_range(-0.5..0.5),
                        rng.random_range(-1.0..1.0),
                    ),
                    burst_speed: rng.random_range(0.5..1.5),
                }
            })
            .collect();

        LogoCloud { points, centroid }
    })
}

fn is_letter_pixel([r, g, b, a]: [u8; 4]) -> bool {
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    200 < a && 230 < min && max - min < 20
}

/// Samples the middle of turbo, skipping its near-black ends so every color pops on a dark backdrop.
#[expect(clippy::disallowed_methods)] // A colormap sample, not a hard-coded UI color.
fn turbo(t: f32, opacity: f32) -> Color32 {
    let [r, g, b, _] = re_renderer::colormap_turbo_srgba(0.1 + 0.8 * t);
    Color32::from_rgb(r, g, b).gamma_multiply(opacity)
}

/// Each letter drops in from above, bounces, then waves through the turbo colormap.
fn paint_bouncing_text(
    painter: &Painter,
    text: &str,
    font_id: &FontId,
    center: Pos2,
    t: f32,
    opacity: f32,
) {
    let galleys: Vec<_> = text
        .chars()
        .map(|c| painter.layout_no_wrap(c.to_string(), font_id.clone(), Color32::WHITE))
        .collect();
    let total_width: f32 = galleys.iter().map(|galley| galley.size().x).sum();
    let height = font_id.size;

    let mut x = center.x - 0.5 * total_width;
    for (i, galley) in galleys.into_iter().enumerate() {
        let width = galley.size().x;
        let t = t - 0.07 * i as f32;
        if 0.0 <= t {
            let drop = 1.0 - bounce_out((t / 0.8).min(1.0));
            let wave = 0.12 * (6.0 * t - 0.6 * i as f32).sin() * (t - 0.8).clamp(0.0, 1.0);
            let pos = Pos2::new(x, center.y - 0.5 * height - (3.0 * drop + wave) * height);
            let color = turbo((0.3 * t + 0.07 * i as f32).fract(), opacity);
            painter.galley_with_override_text_color(pos, galley, color);
        }
        x += width;
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
