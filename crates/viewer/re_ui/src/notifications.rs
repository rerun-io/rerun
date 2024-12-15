use std::time::Duration;

pub use re_log::Level;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotificationLevel {
    Info,
    Warning,
    Error,
    Success,
}

impl From<re_log::Level> for NotificationLevel {
    fn from(value: re_log::Level) -> Self {
        match value {
            re_log::Level::Trace | re_log::Level::Debug | re_log::Level::Info => Self::Info,
            re_log::Level::Warn => Self::Warning,
            re_log::Level::Error => Self::Error,
        }
    }
}

fn is_relevant(level: re_log::Level) -> bool {
    matches!(
        level,
        re_log::Level::Warn | re_log::Level::Error | re_log::Level::Info
    )
}

struct Notification {
    level: NotificationLevel,
    text: String,
    ttl_sec: f64,
}

pub struct NotificationUi {
    /// State of every notification.
    ///
    /// Notifications are stored in order of ascending age, so the latest one is at the end.
    data: Vec<Notification>,

    /// Panel that shows all notifications.
    panel: NotificationPanel,

    /// Toasts that show up for a short time.
    toasts: Toasts,
}

impl Default for NotificationUi {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationUi {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            panel: NotificationPanel::new(),
            toasts: Toasts::new(),
        }
    }

    pub fn add(&mut self, level: Level, text: impl Into<String>) {
        if !is_relevant(level) {
            return;
        }

        self.data.push(Notification {
            level: level.into(),
            text: text.into(),
            ttl_sec: TOAST_TTL_SEC,
        });
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.data.push(Notification {
            level: NotificationLevel::Success,
            text: text.into(),
            ttl_sec: TOAST_TTL_SEC,
        });
    }

    pub fn show(&mut self, egui_ctx: &egui::Context) {
        self.panel.show(egui_ctx, &self.data[..]);

        if self.panel.is_visible {
            for notification in &mut self.data {
                notification.ttl_sec = 0.0;
            }

            self.toasts.show(egui_ctx, &mut self.data[..]);
        }

        if let Some(notification) = self.data.last() {
            if notification.ttl_sec.is_finite() && notification.ttl_sec > 0.0 {
                egui_ctx.request_repaint_after(Duration::from_secs_f64(notification.ttl_sec));
            }
        }
    }
}

struct NotificationPanel {
    id: egui::Id,
    is_visible: bool,
}

impl NotificationPanel {
    fn new() -> Self {
        Self {
            id: egui::Id::new("__notifications"),
            is_visible: true,
        }
    }

    fn show(&self, egui_ctx: &egui::Context, notifications: &[Notification]) {
        if !self.is_visible {
            return;
        }

        let panel_width = 400.0;
        let panel_max_height = 320.0;

        egui::Area::new(self.id)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 32.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .movable(false)
            .show(egui_ctx, |ui| {
                egui::Frame::window(ui.style())
                    .rounding(0.0)
                    .show(ui, |ui| {
                        ui.set_width(panel_width);
                        ui.label("Notifications");
                        egui::ScrollArea::vertical()
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .min_scrolled_height(panel_max_height)
                            .show(ui, |ui| {
                                for Notification { level, text, .. } in notifications.iter().rev() {
                                    ui.horizontal(|ui| {
                                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                        ui.set_max_width(panel_width);
                                        ui.spacing_mut().item_spacing = egui::Vec2::splat(5.0);
                                        log_level_icon(ui, *level);
                                        ui.label(format!("{level:?}: {text}"));
                                    });
                                }
                            });
                    });
            });
    }
}

const TOAST_TTL_SEC: f64 = 4.0;

struct Toasts {
    id: egui::Id,
}

impl Default for Toasts {
    fn default() -> Self {
        Self::new()
    }
}

impl Toasts {
    fn new() -> Self {
        Self {
            id: egui::Id::new("__toasts"),
        }
    }

    /// Shows and updates all toasts
    fn show(&mut self, egui_ctx: &egui::Context, notifications: &mut [Notification]) {
        let Self { id } = self;

        let dt = egui_ctx.input(|i| i.unstable_dt) as f64;
        let mut offset = egui::vec2(-8.0, 8.0);

        for (i, notification) in notifications
            .iter_mut()
            .filter(|n| n.ttl_sec > 0.0)
            .enumerate()
        {
            let response = egui::Area::new(id.with(i))
                .anchor(egui::Align2::RIGHT_TOP, offset)
                .order(egui::Order::Foreground)
                .interactable(true)
                .movable(false)
                .show(egui_ctx, |ui| {
                    show_notification_toast(ui, notification);
                })
                .response;

            if !response.hovered() {
                notification.ttl_sec = (notification.ttl_sec - dt).max(0.0);
            }

            let response = response.on_hover_text("Click to close and copy contents");

            if response.clicked() {
                egui_ctx.output_mut(|o| o.copied_text = notification.text.clone());
                notification.ttl_sec = 0.0;
            }

            offset.y += response.rect.height() + 8.0;
        }
    }
}

fn show_notification_toast(ui: &mut egui::Ui, notification: &Notification) -> egui::Response {
    egui::Frame::window(ui.style())
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.set_max_width(400.0);
                ui.spacing_mut().item_spacing = egui::Vec2::splat(5.0);
                log_level_icon(ui, notification.level);
                ui.label(notification.text.clone());
            })
        })
        .response
}

fn log_level_icon(ui: &mut egui::Ui, level: NotificationLevel) {
    let (icon, icon_color) = match level {
        NotificationLevel::Info => ("ℹ", crate::INFO_COLOR),
        NotificationLevel::Warning => ("⚠", ui.style().visuals.warn_fg_color),
        NotificationLevel::Error => ("❗", ui.style().visuals.error_fg_color),
        NotificationLevel::Success => ("✔", crate::SUCCESS_COLOR),
    };
    ui.label(egui::RichText::new(icon).color(icon_color));
}
