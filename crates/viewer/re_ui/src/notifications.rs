use std::time::Duration;

use egui::NumExt as _;
pub use re_log::Level;
use time::OffsetDateTime;

use crate::design_tokens;
use crate::icons;
use crate::ColorToken;
use crate::Scale;
use crate::UiExt;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationLevel {
    Info = 0,
    Success = 1,
    Warning = 2,
    Error = 3,
}

impl NotificationLevel {
    fn color(&self, ui: &egui::Ui) -> egui::Color32 {
        match self {
            Self::Info => crate::INFO_COLOR,
            Self::Warning => ui.style().visuals.warn_fg_color,
            Self::Error => ui.style().visuals.error_fg_color,
            Self::Success => crate::SUCCESS_COLOR,
        }
    }
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

fn is_relevant(target: &str, level: re_log::Level) -> bool {
    let is_rerun_crate = target.starts_with("rerun") || target.starts_with("re_");
    if !is_rerun_crate {
        return false;
    }

    matches!(
        level,
        re_log::Level::Warn | re_log::Level::Error | re_log::Level::Info
    )
}

fn notification_panel_popup_id() -> egui::Id {
    egui::Id::new("notification_panel_popup")
}

pub fn notification_toggle_button(ui: &mut egui::Ui, notification_ui: &mut NotificationUi) {
    let popup_id = notification_panel_popup_id();

    let is_panel_visible = ui.memory(|mem| mem.is_popup_open(popup_id));
    let button_response =
        ui.medium_icon_toggle_button(&icons::NOTIFICATION, &mut is_panel_visible.clone());

    if button_response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }

    if let Some(level) = notification_ui.unread_notification_level {
        let pos = button_response.rect.right_top() + egui::vec2(-2.0, 2.0);
        let radius = 3.0;
        let color = level.color(ui);
        ui.painter().circle_filled(pos, radius, color);
    }

    notification_ui.ui(ui.ctx(), &button_response);
}

struct Notification {
    level: NotificationLevel,
    text: String,

    /// When this notification was added to the list.
    created_at: OffsetDateTime,

    /// Time to live for toasts, the notification itself lives until dismissed.
    toast_ttl: Duration,

    /// Whether this notification has been read.
    is_unread: bool,
}

pub struct NotificationUi {
    /// State of every notification.
    ///
    /// Notifications are stored in order of ascending `created_at`, so the latest one is at the end.
    notifications: Vec<Notification>,

    unread_notification_level: Option<NotificationLevel>,
    was_open_last_frame: bool,

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
            notifications: Vec::new(),
            unread_notification_level: None,
            was_open_last_frame: false,
            panel: NotificationPanel::new(),
            toasts: Toasts::new(),
        }
    }

    pub fn unread_notification_level(&self) -> Option<NotificationLevel> {
        self.unread_notification_level
    }

    pub fn add_log(&mut self, message: re_log::LogMsg) {
        if !is_relevant(&message.target, message.level) {
            return;
        }

        self.push(message.level.into(), message.msg);
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.push(NotificationLevel::Success, text.into());
    }

    fn push(&mut self, level: NotificationLevel, text: String) {
        self.notifications.push(Notification {
            level,
            text,

            created_at: now(),
            toast_ttl: base_ttl(),
            is_unread: true,
        });

        if Some(level) > self.unread_notification_level {
            self.unread_notification_level = Some(level);
        }
    }

    fn ui(&mut self, egui_ctx: &egui::Context, button_response: &egui::Response) {
        let is_panel_visible =
            egui_ctx.memory(|mem| mem.is_popup_open(notification_panel_popup_id()));
        if is_panel_visible {
            // Dismiss all toasts when opening panel
            self.unread_notification_level = None;
            for notification in &mut self.notifications {
                notification.toast_ttl = Duration::ZERO;
            }
        }
        if !is_panel_visible && self.was_open_last_frame {
            // Mark all as read after closing panel
            for notification in &mut self.notifications {
                notification.is_unread = false;
            }
        }
        self.was_open_last_frame = is_panel_visible;

        if is_panel_visible {
            let panel_response = self.panel.show(egui_ctx, &mut self.notifications);
            let escape_pressed =
                egui_ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if escape_pressed
                || button_response.clicked_elsewhere() && panel_response.clicked_elsewhere()
            {
                egui_ctx.memory_mut(|mem| mem.close_popup());
            }
        }

        self.toasts.show(egui_ctx, &mut self.notifications[..]);
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
    ) -> egui::Response {
        let panel_width = 356.0;
        let panel_max_height = (egui_ctx.screen_rect().height() - 100.0)
            .at_least(0.0)
            .at_most(640.0);

        let mut to_dismiss = None;

        let notification_list = |ui: &mut egui::Ui| {
            if notifications.is_empty() {
                ui.label(
                    design_tokens()
                        .text("No notifications yet.", ColorToken::gray(Scale::S450))
                        .weak(),
                );

                return;
            }

            for (i, notification) in notifications.iter().enumerate().rev() {
                show_notification(ui, notification, DisplayMode::Panel, || {
                    to_dismiss = Some(i);
                });
            }
        };

        let mut dismiss_all = false;

        let response = egui::Area::new(self.id)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 32.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .movable(false)
            .show(egui_ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(design_tokens().color(ColorToken::gray(Scale::S150)))
                    .rounding(8.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_width(panel_width);
                        ui.set_max_height(panel_max_height);

                        ui.horizontal_top(|ui| {
                            if !notifications.is_empty() {
                                ui.label(format!("Notifications ({})", notifications.len()));
                            } else {
                                ui.label("Notifications");
                            }
                            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                                if ui.small_icon_button(&icons::CLOSE).clicked() {
                                    ui.memory_mut(|mem| mem.close_popup());
                                }
                            });
                        });
                        egui::ScrollArea::vertical()
                            .min_scrolled_height(panel_max_height / 2.0)
                            .max_height(panel_max_height)
                            .show(ui, notification_list);

                        if !notifications.is_empty() {
                            ui.horizontal_top(|ui| {
                                if ui.button("Dismiss all").clicked() {
                                    dismiss_all = true;
                                };
                            });
                        }
                    });
            })
            .response;

        if dismiss_all {
            notifications.clear();
        } else if let Some(to_dismiss) = to_dismiss {
            notifications.remove(to_dismiss);
        }

        response
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

        let mut first_nonzero_ttl = None;

        for (i, notification) in notifications
            .iter_mut()
            .enumerate()
            .filter(|(_, n)| n.toast_ttl > Duration::ZERO)
        {
            first_nonzero_ttl.get_or_insert(notification.toast_ttl);

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
                if notification.toast_ttl < dt {
                    notification.toast_ttl = Duration::ZERO;
                } else {
                    notification.toast_ttl -= dt;
                }
            }

            let response = response.on_hover_text("Click to close and copy contents");

            if response.clicked() {
                egui_ctx.output_mut(|o| o.copied_text = notification.text.clone());
                notification.toast_ttl = Duration::ZERO;
            }

            offset.y += response.rect.height() + 8.0;
        }

        if let Some(first_nonzero_ttl) = first_nonzero_ttl {
            egui_ctx.request_repaint_after(first_nonzero_ttl);
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
    let background_color = if mode == DisplayMode::Toast || notification.is_unread {
        design_tokens().color(ColorToken::gray(Scale::S200))
    } else {
        design_tokens().color(ColorToken::gray(Scale::S150))
    };

    egui::Frame::window(ui.style())
        .rounding(4.0)
        .inner_margin(10.0)
        .fill(background_color)
        .shadow(egui::Shadow::NONE)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {
                    log_level_icon(ui, notification.level);
                    ui.horizontal_top(|ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        ui.set_width(270.0);
                        ui.label(
                            design_tokens()
                                .text(notification.text.clone(), ColorToken::gray(Scale::S775))
                                .weak(),
                        );
                    });

                    ui.add_space(4.0);
                    if mode == DisplayMode::Panel {
                        notification_age_label(ui, notification);
                    }
                });

                ui.horizontal_top(|ui| {
                    if mode != DisplayMode::Panel {
                        return;
                    }

                    ui.add_space(17.0);
                    if ui.button("Dismiss").clicked() {
                        on_dismiss();
                    }
                });
            })
        })
        .response
}

fn notification_age_label(ui: &mut egui::Ui, notification: &Notification) {
    let age = (now() - notification.created_at).as_seconds_f64();

    let formatted = if age < 10.0 {
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        "now".to_owned()
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
        ui.set_min_width(30.0);
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            ui.label(
                design_tokens()
                    .text(formatted, ColorToken::gray(Scale::S450))
                    .weak(),
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
    ui.painter()
        .circle_filled(rect.center() + egui::vec2(0.0, 2.0), 5.0, color);
}
