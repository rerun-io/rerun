//! # UI for notifications.
//!
//! Notifications are drawn both as a toast for some time when
//! they're first created and in the notification panel.
//!
//! ## Special cased text
//! - If a notifications text contains `"\nDetails:"` the section after that
//!   will be displayed inside a collapsible details header.

use std::time::Duration;

use egui::{NumExt as _, Widget as _};
use jiff::Timestamp;
pub use re_log::Level;

use crate::{UiExt as _, icons};

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

    fn icon(&self) -> &icons::Icon {
        match self {
            Self::Tip | Self::Info => &icons::INFO,
            Self::Success => &icons::SUCCESS,
            Self::Warning => &icons::WARNING,
            Self::Error => &icons::ERROR,
        }
    }

    fn image(&self, ui: &egui::Ui) -> egui::Image<'_> {
        let color = self.color(ui);
        let icon = self.icon();
        icon.as_image().tint(color)
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

    /// if set this notifications will have a collapsible details section.
    details: Option<String>,
    link: Option<Link>,

    /// If set, the notification will NEVER be shown again
    /// if the user has dismissed it.
    permanent_dismiss_id: Option<egui::Id>,

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
            details: None,
            link: None,
            permanent_dismiss_id: None,
            created_at: Timestamp::now(),
            toast_ttl: base_ttl(),
            is_unread: true,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn with_link(mut self, link: Link) -> Self {
        self.link = Some(link);
        self
    }

    // Show no toast - only show when clicking the notification panel!
    pub fn no_toast(mut self) -> Self {
        self.toast_ttl = Duration::ZERO;
        self
    }

    /// If set, the notification will NEVER be shown again
    /// if the user has dismissed it.
    pub fn permanent_dismiss_id(mut self, id: egui::Id) -> Self {
        self.permanent_dismiss_id = Some(id);
        self
    }

    /// Called only when this notification was dismissed on its own.
    fn remember_dismiss(&self, ctx: &egui::Context) {
        if let Some(permanent_dismiss_id) = self.permanent_dismiss_id {
            ctx.data_mut(|data| data.insert_persisted(permanent_dismiss_id, PermaDismissiedMarker));
        }
    }

    /// Did the user already dismiss this during an earlier run?
    fn is_perma_dismissed(&self, ctx: &egui::Context) -> bool {
        self.permanent_dismiss_id.is_some_and(|id| {
            ctx.data_mut(|data| data.get_persisted::<PermaDismissiedMarker>(id))
                .is_some()
        })
    }
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct PermaDismissiedMarker;

enum NotificationReaction {
    Dismissed,
    NeverShowAgain,
}

pub struct NotificationUi {
    ctx: egui::Context,

    /// State of every notification.
    ///
    /// Notifications are stored in order of ascending `created_at`, so the latest one is at the end.
    notifications: Vec<Notification>,

    unread_notification_level: Option<NotificationLevel>,
    was_open_last_frame: bool,

    /// Toasts that show up for a short time.
    toasts: Toasts,
}

impl NotificationUi {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            notifications: Vec::new(),
            unread_notification_level: None,
            was_open_last_frame: false,
            toasts: Toasts::new(),
        }
    }

    pub fn unread_notification_level(&self) -> Option<NotificationLevel> {
        self.unread_notification_level
    }

    /// Given that the log is relevant this creates a notification
    /// based on that log.
    ///
    /// ## Special cased text
    /// - If a notifications text contains `"\nDetails:"` the section after that
    ///   will be displayed inside a collapsible details header.
    pub fn add_log(&mut self, message: re_log::LogMsg) {
        let re_log::LogMsg { level, target, msg } = message;

        if is_relevant(&target, level) {
            let (split_msg, msg_details) = msg.split_once("\nDetails:").unzip();

            let msg = split_msg.unwrap_or(&msg);

            let mut notification = Notification::new(level.into(), msg);

            if let Some(msg_details) = msg_details {
                notification = notification.with_details(msg_details);
            }

            self.add(notification);
        }
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.add(Notification::new(NotificationLevel::Success, text.into()));
    }

    pub fn add(&mut self, notification: Notification) {
        if notification.is_perma_dismissed(&self.ctx) {
            return;
        }

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
                ui.content_rect().right() - gap,
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
        let panel_max_height = (ui.content_rect().height() - 100.0)
            .at_least(0.0)
            .at_most(640.0);

        let mut to_dismiss = None;

        let notification_list = |ui: &mut egui::Ui| {
            if notifications.is_empty() {
                ui.label(egui::RichText::new("No notifications yet.").weak());

                return;
            }

            for (i, notification) in notifications.iter().enumerate().rev() {
                let reaction = show_notification(ui, notification, DisplayMode::Panel).0;
                if reaction.is_some() {
                    to_dismiss = Some(i);
                }
            }
        };

        let mut dismiss_all = false;

        ui.set_width(panel_width);
        ui.set_max_height(panel_max_height);

        ui.horizontal_top(|ui| {
            if !notifications.is_empty() {
                ui.strong(format!("Notifications ({})", notifications.len()));
            } else {
                ui.strong("Notifications");
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
            let removed = notifications.remove(to_dismiss);
            removed.remember_dismiss(ui.ctx());
        }
    }

    /// Show floating toast notifications of recent log messages.
    pub fn show_toasts(&mut self, egui_ctx: &egui::Context) {
        self.toasts.show(egui_ctx, &mut self.notifications[..]);
    }
}

fn base_ttl() -> Duration {
    Duration::from_secs(4)
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
        let dt = Duration::try_from_secs_f32(egui_ctx.input(|i| i.unstable_dt))
            .unwrap_or(std::time::Duration::from_millis(100));

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
                    show_notification(ui, notification, DisplayMode::Toast);
                })
                .response;

            if !response.hovered()
                && !egui_ctx.rect_contains_pointer(response.layer_id, response.interact_rect)
            {
                notification.toast_ttl = notification.toast_ttl.saturating_sub(dt);
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
) -> (Option<NotificationReaction>, egui::Response) {
    let Notification {
        level,
        text,
        details,
        link,
        permanent_dismiss_id,
        created_at,
        toast_ttl: _,
        is_unread,
    } = notification;

    let background_color = if mode == DisplayMode::Toast || *is_unread {
        ui.tokens().notification_background_color
    } else {
        ui.tokens().notification_panel_background_color
    };

    let mut reaction = None;

    let response = egui::Frame::window(ui.style())
        .corner_radius(4)
        .inner_margin(10.0)
        .fill(background_color)
        .shadow(egui::Shadow::NONE)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {
                    ui.add(level.image(ui));

                    ui.vertical(|ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        ui.set_width(270.0);
                        ui.label(text);

                        if let Some(details) = details {
                            ui.collapsing_header("Details", false, |ui| ui.label(details));
                        }
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
                            if show_dismiss {
                                if permanent_dismiss_id.is_some() {
                                    if ui.button("Don't show again").clicked() {
                                        reaction = Some(NotificationReaction::NeverShowAgain);
                                    }
                                } else {
                                    //
                                    if ui.button("Dismiss").clicked() {
                                        reaction = Some(NotificationReaction::Dismissed);
                                    }
                                }
                            }
                        },
                    );
                }
            })
        })
        .response;

    (reaction, response)
}

fn notification_age_label(ui: &mut egui::Ui, created_at: Timestamp) {
    // TODO(emilk): use short_duration_ui

    let age = Timestamp::now().duration_since(created_at).as_secs_f64();

    let formatted = if age < 10.0 {
        ui.request_repaint_after(Duration::from_secs(1));

        "now".to_owned()
    } else if age < 60.0 {
        ui.request_repaint_after(Duration::from_secs(1));

        format!("{age:.0}s")
    } else {
        ui.request_repaint_after(Duration::from_secs(60));

        created_at.strftime("%H:%M").to_string()
    };

    ui.horizontal_top(|ui| {
        ui.set_min_width(30.0);
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(formatted).weak())
                    .wrap_mode(egui::TextWrapMode::Extend),
            )
            .on_hover_text(created_at.to_string());
        });
    });
}
