use std::time::Duration;

use egui::{NumExt as _, Widget as _};
use jiff::Timestamp;

pub use re_log::Level;

use crate::{UiExt, icons};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationLevel {
    Tip,
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationLevel {
    fn color(&self, ui: &egui::Ui) -> egui::Color32 {
        match self {
            Self::Tip | Self::Info => ui.tokens().info_text_color,
            Self::Warning => ui.style().visuals.warn_fg_color,
            Self::Error => ui.style().visuals.error_fg_color,
            Self::Success => ui.tokens().success_text_color,
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

/// A link to some URL.
pub struct Link {
    pub text: String,
    pub url: String,
}

impl egui::Widget for &Link {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Link { text, url } = self;
        ui.re_hyperlink(text, url, true)
    }
}

impl egui::Widget for Link {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self { text, url } = self;
        ui.re_hyperlink(text, url, true)
    }
}

/// A notification to show the user
pub struct Notification {
    level: NotificationLevel,
    text: String,
    link: Option<Link>,

    /// When this notification was added to the list.
    created_at: Timestamp,

    /// Time to live for toasts, the notification itself lives until dismissed.
    toast_ttl: Duration,

    /// Whether this notification has been read.
    is_unread: bool,
}

impl Notification {
    pub fn new(level: NotificationLevel, text: impl Into<String>) -> Self {
        Self {
            level,
            text: text.into(),
            link: None,
            created_at: Timestamp::now(),
            toast_ttl: base_ttl(),
            is_unread: true,
        }
    }

    pub fn with_link(mut self, link: Link) -> Self {
        self.link = Some(link);
        self
    }
}

pub struct NotificationUi {
    /// State of every notification.
    ///
    /// Notifications are stored in order of ascending `created_at`, so the latest one is at the end.
    notifications: Vec<Notification>,

    unread_notification_level: Option<NotificationLevel>,
    was_open_last_frame: bool,

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
            toasts: Toasts::new(),
        }
    }

    pub fn unread_notification_level(&self) -> Option<NotificationLevel> {
        self.unread_notification_level
    }

    pub fn add_log(&mut self, message: re_log::LogMsg) {
        let re_log::LogMsg { level, target, msg } = message;

        if !is_relevant(&target, level) {
            return;
        }

        self.add(Notification::new(level.into(), msg));
    }
    pub fn success(&mut self, text: impl Into<String>) {
        self.add(Notification::new(NotificationLevel::Success, text.into()));
    }

    pub fn add(&mut self, notification: Notification) {
        if Some(notification.level) > self.unread_notification_level {
            self.unread_notification_level = Some(notification.level);
        }
        self.notifications.push(notification);
    }

    /// A little bell-like button, that shows recent notifications when clicked.
    pub fn notification_toggle_button(&mut self, ui: &mut egui::Ui) {
        let popup_id = notification_panel_popup_id();

        let is_panel_visible = egui::Popup::is_id_open(ui.ctx(), popup_id);
        let button_response = ui.medium_icon_toggle_button(
            &icons::NOTIFICATION,
            "Notification toggle",
            &mut is_panel_visible.clone(),
        );

        if let Some(level) = self.unread_notification_level {
            let pos = button_response.rect.right_top() + egui::vec2(-2.0, 2.0);
            let radius = 3.0;
            let color = level.color(ui);
            ui.painter().circle_filled(pos, radius, color);
        }

        let gap = 2.0;

        let mut is_visible = false;

        egui::Popup::from_toggle_button_response(&button_response)
            .id(popup_id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(ui.tokens().popup_frame(ui.style()))
            // Put the popup below the button, but all the way to the right of the screen:
            .anchor(egui::PopupAnchor::Position(egui::pos2(
                ui.ctx().screen_rect().right() - gap,
                ui.max_rect().bottom() + gap,
            )))
            .align(egui::RectAlign::BOTTOM_END)
            .show(|ui| {
                self.popup_contents(ui);
                is_visible = true;
            });

        if is_panel_visible {
            // Dismiss all toasts when opening popup
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
    }

    fn popup_contents(&mut self, ui: &mut egui::Ui) {
        let notifications = &mut self.notifications;

        let panel_width = 356.0;
        let panel_max_height = (ui.ctx().screen_rect().height() - 100.0)
            .at_least(0.0)
            .at_most(640.0);

        let mut to_dismiss = None;

        let notification_list = |ui: &mut egui::Ui| {
            if notifications.is_empty() {
                ui.label(egui::RichText::new("No notifications yet.").weak());

                return;
            }

            for (i, notification) in notifications.iter().enumerate().rev() {
                show_notification(ui, notification, DisplayMode::Panel, || {
                    to_dismiss = Some(i);
                });
            }
        };

        let mut dismiss_all = false;

        ui.set_width(panel_width);
        ui.set_max_height(panel_max_height);

        ui.horizontal_top(|ui| {
            if !notifications.is_empty() {
                ui.label(format!("Notifications ({})", notifications.len()));
            } else {
                ui.label("Notifications");
            }
            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                if ui.small_icon_button(&icons::CLOSE, "Close").clicked() {
                    ui.close();
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
                }
            });
        }

        if dismiss_all {
            notifications.clear();
        } else if let Some(to_dismiss) = to_dismiss {
            notifications.remove(to_dismiss);
        }
    }

    /// Show floating toast notifications of recent log messages.
    pub fn show_toasts(&mut self, egui_ctx: &egui::Context) {
        self.toasts.show(egui_ctx, &mut self.notifications[..]);
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
                if let Some(link) = &notification.link {
                    egui_ctx.open_url(egui::OpenUrl::new_tab(link.url.clone()));
                } else {
                    egui_ctx.copy_text(notification.text.clone());
                }
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
    let Notification {
        level,
        text,
        link,
        created_at,
        toast_ttl: _,
        is_unread,
    } = notification;

    let background_color = if mode == DisplayMode::Toast || *is_unread {
        ui.tokens().notification_background_color
    } else {
        ui.tokens().notification_panel_background_color
    };

    egui::Frame::window(ui.style())
        .corner_radius(4)
        .inner_margin(10.0)
        .fill(background_color)
        .shadow(egui::Shadow::NONE)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {
                    log_level_icon(ui, *level);
                    ui.horizontal_top(|ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        ui.set_width(270.0);
                        ui.label(egui::RichText::new(text.clone()));
                    });

                    ui.add_space(4.0);
                    if mode == DisplayMode::Panel {
                        notification_age_label(ui, *created_at);
                    }
                });

                let show_dismiss = mode == DisplayMode::Panel;
                let show_bottom_bar = show_dismiss || link.is_some();

                if show_bottom_bar {
                    egui::Sides::new().show(
                        ui,
                        |ui| {
                            if let Some(link) = link {
                                link.ui(ui);
                            }
                        },
                        |ui| {
                            if show_dismiss && ui.button("Dismiss").clicked() {
                                on_dismiss();
                            }
                        },
                    );
                }
            })
        })
        .response
}

fn notification_age_label(ui: &mut egui::Ui, created_at: Timestamp) {
    // TODO(emilk): use short_duration_ui

    let age = Timestamp::now().duration_since(created_at).as_secs_f64();

    let formatted = if age < 10.0 {
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        "now".to_owned()
    } else if age < 60.0 {
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        format!("{age:.0}s")
    } else {
        ui.ctx().request_repaint_after(Duration::from_secs(60));

        created_at.strftime("%H:%M").to_string()
    };

    ui.horizontal_top(|ui| {
        ui.set_min_width(30.0);
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            ui.label(egui::RichText::new(formatted).weak())
                .on_hover_text(created_at.to_string());
        });
    });
}

fn log_level_icon(ui: &mut egui::Ui, level: NotificationLevel) {
    let color = level.color(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center() + egui::vec2(0.0, 2.0), 5.0, color);
}
