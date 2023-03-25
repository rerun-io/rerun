///! A toast notification system for egui, roughly based on <https://github.com/urholaukkarinen/egui-toast>.
use std::collections::HashMap;

use egui::Color32;

pub const INFO_COLOR: Color32 = Color32::from_rgb(0, 155, 255);
pub const WARNING_COLOR: Color32 = Color32::from_rgb(255, 212, 0);
pub const ERROR_COLOR: Color32 = Color32::from_rgb(255, 32, 0);
pub const SUCCESS_COLOR: Color32 = Color32::from_rgb(0, 255, 32);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ToastKind {
    Info,
    Warning,
    Error,
    Success,
    Custom(u32),
}

#[derive(Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub options: ToastOptions,
}

#[derive(Copy, Clone)]
pub struct ToastOptions {
    /// This can be used to show or hide the toast type icon.
    pub show_icon: bool,

    /// Time to live in seconds.
    pub ttl_sec: f64,
}

impl ToastOptions {
    pub fn with_ttl_in_seconds(ttl_sec: f64) -> Self {
        Self {
            show_icon: true,
            ttl_sec,
        }
    }
}

impl Toast {
    pub fn close(&mut self) {
        self.options.ttl_sec = 0.0;
    }
}

pub type ToastContents = dyn Fn(&mut egui::Ui, &mut Toast) -> egui::Response;

pub struct Toasts {
    id: egui::Id,
    custom_toast_contents: HashMap<ToastKind, Box<ToastContents>>,
    toasts: Vec<Toast>,
}

impl Default for Toasts {
    fn default() -> Self {
        Self::new()
    }
}

impl Toasts {
    pub fn new() -> Self {
        Self {
            id: egui::Id::new("__toasts"),
            custom_toast_contents: Default::default(),
            toasts: Vec::new(),
        }
    }

    /// Adds a new toast
    pub fn add(&mut self, toast: Toast) -> &mut Self {
        self.toasts.push(toast);
        self
    }

    /// Shows and updates all toasts
    pub fn show(&mut self, egui_ctx: &egui::Context) {
        let Self {
            id,
            custom_toast_contents,
            toasts,
        } = self;

        let dt = egui_ctx.input(|i| i.unstable_dt) as f64;

        toasts.retain(|toast| 0.0 < toast.options.ttl_sec);

        let mut offset = egui::vec2(-8.0, 8.0);

        for (i, toast) in toasts.iter_mut().enumerate() {
            let response = egui::Area::new(id.with(i))
                .anchor(egui::Align2::RIGHT_TOP, offset)
                .order(egui::Order::Foreground)
                .interactable(true)
                .movable(false)
                .show(egui_ctx, |ui| {
                    if let Some(add_contents) = custom_toast_contents.get_mut(&toast.kind) {
                        add_contents(ui, toast);
                    } else {
                        default_toast_contents(ui, toast);
                    };
                })
                .response;

            let response = response
                .interact(egui::Sense::click())
                .on_hover_text("Click to close and copy contents");

            if !response.hovered() {
                toast.options.ttl_sec -= dt;
                if toast.options.ttl_sec.is_finite() {
                    egui_ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                        toast.options.ttl_sec.max(0.0),
                    ));
                }
            }

            if response.clicked() {
                egui_ctx.output_mut(|o| o.copied_text = toast.text.clone());
                toast.close();
            }

            offset.y += response.rect.height() + 8.0;
        }
    }
}

fn default_toast_contents(ui: &mut egui::Ui, toast: &mut Toast) -> egui::Response {
    egui::Frame::window(ui.style())
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().wrap = Some(true);
                ui.set_max_width(400.0);
                ui.spacing_mut().item_spacing = egui::Vec2::splat(5.0);

                if toast.options.show_icon {
                    let (icon, icon_color) = match toast.kind {
                        ToastKind::Warning => ("⚠", WARNING_COLOR),
                        ToastKind::Error => ("❗", ERROR_COLOR),
                        ToastKind::Success => ("✔", SUCCESS_COLOR),
                        _ => ("ℹ", INFO_COLOR),
                    };
                    ui.label(egui::RichText::new(icon).color(icon_color));
                }
                ui.label(toast.text.clone());
            })
        })
        .response
}
