use std::time::Duration;

use egui::hex_color;
pub use re_log::Level;
use time::OffsetDateTime;

use crate::icons;
use crate::UiExt;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

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

pub fn notification_toggle_button(
    ui: &mut egui::Ui,
    show_notification_panel: &mut bool,
    has_unread_notifications: bool,
) {
    let response = ui.medium_icon_toggle_button(&icons::NOTIFICATION, show_notification_panel);

    if has_unread_notifications {
        let mut pos = response.rect.right_top();
        pos.x -= 2.0;
        pos.y += 2.0;
        ui.painter().circle_filled(pos, 3.0, hex_color!("#ab0037"));
    }
}

struct Notification {
    level: NotificationLevel,
    text: String,

    created_at: OffsetDateTime,
    ttl: Duration,
}

pub struct NotificationUi {
    /// State of every notification.
    ///
    /// Notifications are stored in order of ascending age, so the latest one is at the end.
    data: Vec<Notification>,

    has_unread_notifications: bool,

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
            has_unread_notifications: false,
            panel: NotificationPanel::new(),
            toasts: Toasts::new(),
        }
    }

    pub fn has_unread_notifications(&self) -> bool {
        self.has_unread_notifications
    }

    pub fn add(&mut self, level: Level, text: impl Into<String>) {
        if !is_relevant(level) {
            return;
        }

        self.data.push(Notification {
            level: level.into(),
            text: text.into(),

            created_at: now(),
            ttl: base_ttl(),
        });
        self.has_unread_notifications = true;
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.data.push(Notification {
            level: NotificationLevel::Success,
            text: text.into(),

            created_at: now(),
            ttl: base_ttl(),
        });
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context, is_panel_visible: &mut bool) {
        if *is_panel_visible {
            let escape_pressed =
                egui_ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if escape_pressed {
                *is_panel_visible = false;
            }

            self.has_unread_notifications = false;
        }

        self.panel.show(egui_ctx, &mut self.data, is_panel_visible);
        self.toasts.show(egui_ctx, &mut self.data[..]);

        if let Some(notification) = self.data.last() {
            if !notification.ttl.is_zero() {
                egui_ctx.request_repaint_after(notification.ttl);
            }
        }
    }
}

struct NotificationPanel {
    id: egui::Id,
}

impl NotificationPanel {
    fn new() -> Self {
        Self {
            id: egui::Id::new("__notifications"),
        }
    }

    fn show(
        &self,
        egui_ctx: &egui::Context,
        notifications: &mut Vec<Notification>,
        is_panel_visible: &mut bool,
    ) {
        if !*is_panel_visible {
            return;
        }

        for notification in notifications.iter_mut() {
            notification.ttl = Duration::ZERO;
        }

        let panel_width = 356.0;
        let panel_max_height = 640.0;

        let mut to_dismiss = None;

        let notification_list = |ui: &mut egui::Ui| {
            if notifications.is_empty() {
                ui.label(
                    egui::RichText::new("Nothing here!")
                        .weak()
                        .color(hex_color!("#636b6f")),
                );

                return;
            }

            for (i, notification) in notifications.iter().enumerate().rev() {
                show_notification(ui, notification, DisplayMode::Panel, || {
                    to_dismiss = Some(i);
                });
            }
        };

        let mut clear = false;

        egui::Area::new(self.id)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 32.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .movable(false)
            .show(egui_ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(hex_color!("#141819"))
                    .rounding(8.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_width(panel_width);
                        ui.horizontal_top(|ui| {
                            ui.label("Notifications");
                            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                                if ui.small_icon_button(&icons::CLOSE).clicked() {
                                    *is_panel_visible = false;
                                }
                            });
                        });
                        egui::ScrollArea::vertical()
                            .min_scrolled_height(panel_max_height)
                            .max_height(panel_max_height)
                            .show(ui, notification_list);

                        if !notifications.is_empty() {
                            ui.horizontal_top(|ui| {
                                if ui.button("Clear all").clicked() {
                                    clear = true;
                                };
                            });
                        }
                    });
            });

        if clear {
            notifications.clear();
        } else if let Some(to_dismiss) = to_dismiss {
            notifications.remove(to_dismiss);
        }
    }
}

fn base_ttl() -> Duration {
    Duration::from_secs_f64(4.0)
}

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
    fn show(&self, egui_ctx: &egui::Context, notifications: &mut [Notification]) {
        let dt = Duration::from_secs_f32(egui_ctx.input(|i| i.unstable_dt));
        let mut offset = egui::vec2(-8.0, 32.0);

        for (i, notification) in notifications
            .iter_mut()
            .enumerate()
            .filter(|(_, n)| n.ttl > Duration::ZERO)
        {
            let response = egui::Area::new(self.id.with(i))
                .anchor(egui::Align2::RIGHT_TOP, offset)
                .order(egui::Order::Foreground)
                .interactable(true)
                .movable(false)
                .show(egui_ctx, |ui| {
                    show_notification(ui, notification, DisplayMode::Toast, || {})
                })
                .response;

            if !response.hovered() {
                if notification.ttl < dt {
                    notification.ttl = Duration::ZERO;
                } else {
                    notification.ttl -= dt;
                }
            }

            let response = response.on_hover_text("Click to close and copy contents");

            if response.clicked() {
                egui_ctx.output_mut(|o| o.copied_text = notification.text.clone());
                notification.ttl = Duration::ZERO;
            }

            offset.y += response.rect.height() + 8.0;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DisplayMode {
    Panel,
    Toast,
}

fn show_notification(
    ui: &mut egui::Ui,
    notification: &Notification,
    mode: DisplayMode,
    mut on_dismiss: impl FnMut(),
) -> egui::Response {
    egui::Frame::window(ui.style())
        .rounding(4.0)
        .inner_margin(10.0)
        .fill(hex_color!("#1c2123"))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let text_response = ui
                    .horizontal_top(|ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        ui.set_max_width(300.0);
                        ui.spacing_mut().item_spacing.x = 8.0;
                        log_level_icon(ui, notification.level);
                        ui.label(
                            egui::RichText::new(notification.text.clone())
                                .color(hex_color!("#cad8de"))
                                .weak(),
                        );

                        ui.spacing_mut().item_spacing.x = 4.0;
                        if mode == DisplayMode::Panel {
                            notification_age_label(ui, notification);
                        }
                    })
                    .response;

                let controls_response = ui
                    .horizontal_top(|ui| {
                        if mode != DisplayMode::Panel {
                            return;
                        }

                        ui.add_space(17.0);
                        if ui.button("Dismiss").clicked() {
                            on_dismiss();
                        }
                    })
                    .response;

                text_response.union(controls_response)
            })
        })
        .response
}

fn notification_age_label(ui: &mut egui::Ui, notification: &Notification) {
    let age = (now() - notification.created_at).as_seconds_f64();

    let formatted = if age < 10.0 {
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        "just now".to_owned()
    } else if age < 60.0 {
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        format!("{age:.0}s")
    } else {
        ui.ctx().request_repaint_after(Duration::from_secs(60));

        notification
            .created_at
            .format(&time::macros::format_description!("[hour]:[minute]"))
            .unwrap_or_default()
    };

    ui.horizontal_top(|ui| {
        ui.set_min_width(52.0);
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            ui.label(
                egui::RichText::new(formatted)
                    .weak()
                    .color(hex_color!("#636b6f")),
            )
            .on_hover_text(format!("{}", notification.created_at));
        });
    });
}

fn log_level_icon(ui: &mut egui::Ui, level: NotificationLevel) {
    let color = match level {
        NotificationLevel::Info => crate::INFO_COLOR,
        NotificationLevel::Warning => ui.style().visuals.warn_fg_color,
        NotificationLevel::Error => ui.style().visuals.error_fg_color,
        NotificationLevel::Success => crate::SUCCESS_COLOR,
    };

    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    let mut pos = rect.center();
    pos.y += 2.0;
    ui.painter().circle_filled(pos, 5.0, color);
}
