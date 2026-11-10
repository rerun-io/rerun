//! # UI for notifications.
//!
//! Notifications are drawn both as a toast for some time when
//! they're first created and in the notification panel.
//!
//! ## Special cased text
//! - If a notifications text has details (see [`re_error::StructuredError`]), those are shown
//!   inside a collapsible details header, one per line. Structured log fields go in there too.
//! - URLs in notification text are rendered as inline clickable links.

use std::collections::VecDeque;
use std::time::Duration;

use egui::{NumExt as _, Widget as _};
use jiff::Timestamp;
pub use re_log::Level;

use crate::{UiExt as _, icons};

/// How many earlier occurrences of a repeated notification keep their details.
///
/// A failing request retried in a loop can repeat a notification thousands of times, and every
/// occurrence carries its own trace id, so the details of old ones are dropped (but still counted).
const MAX_EARLIER_OCCURRENCES: usize = 100;

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

    fn name(&self) -> &'static str {
        match self {
            Self::Tip => "tip",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
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
            re_log::Level::TRACE | re_log::Level::DEBUG | re_log::Level::INFO => Self::Info,
            re_log::Level::WARN => Self::Warning,
            re_log::Level::ERROR => Self::Error,
        }
    }
}

fn is_relevant(target: &str, level: re_log::Level) -> bool {
    let is_rerun_crate = target.starts_with("rerun") || target.starts_with("re_");
    if !is_rerun_crate {
        return false;
    }

    // There is often an overlap between the info messages from`re_server`
    // and the viewer. Since the viewer usually has more context it is better
    // suited to inform the user. We suppress info messages from `re_server`
    // to avoid spamming.
    if level == re_log::Level::INFO && (target == "re_server" || target.starts_with("re_server::"))
    {
        return false;
    }

    matches!(
        level,
        re_log::Level::WARN | re_log::Level::ERROR | re_log::Level::INFO
    )
}

/// Whether the event may become a notification, or asked to stay log-only.
///
/// See [`re_log::GUI_NOTIFICATION_FIELD`].
fn show_in_gui(fields: &[(&'static str, re_log::FieldValue)]) -> bool {
    !fields.iter().any(|(key, value)| {
        *key == re_log::GUI_NOTIFICATION_FIELD && matches!(value, re_log::FieldValue::Bool(false))
    })
}

fn notification_panel_popup_id() -> egui::Id {
    egui::Id::unique("notification_panel_popup")
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

    /// If non-empty, this notification will have a collapsible details section,
    /// with one line per detail.
    ///
    /// Structured key-value fields end up here too, as `key: value` lines.
    ///
    /// For a repeated notification, these are the details of the latest occurrence.
    details: Vec<String>,

    /// The details of earlier occurrences of this notification, oldest first.
    ///
    /// Repeats are merged into one notification by [`NotificationUi::add`].
    /// At most [`MAX_EARLIER_OCCURRENCES`] are kept.
    earlier_details: VecDeque<Vec<String>>,

    /// How many times this notification was added, including the occurrences whose details were
    /// dropped from [`Self::earlier_details`].
    count: usize,

    link: Option<Link>,

    /// If set, the notification will NEVER be shown again
    /// if the user has dismissed it.
    permanent_dismiss_id: Option<egui::Id>,

    /// When this notification was last added to the list.
    created_at: Timestamp,

    /// Time to live for toasts, the notification itself lives until dismissed.
    toast_ttl: Duration,

    /// Whether this notification has been read.
    is_unread: bool,

    /// A unique id that just this notification has.
    unique_id: u64,
}

impl Notification {
    pub fn new(level: NotificationLevel, text: impl Into<String>) -> Self {
        Self {
            level,
            text: text.into(),
            details: Vec::new(),
            earlier_details: VecDeque::new(),
            count: 1,
            link: None,
            permanent_dismiss_id: None,
            created_at: Timestamp::now(),
            toast_ttl: base_ttl(),
            is_unread: true,
            // Filled in later when added to the notification ui.
            unique_id: 0,
        }
    }

    pub fn level(&self) -> NotificationLevel {
        self.level
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The full text contents, including the details of every kept occurrence.
    ///
    /// The lines all occurrences share come first, then one line per occurrence with the rest of
    /// its details, newest first, so each detail stays with the occurrence it belongs to.
    fn copy_text(&self) -> String {
        let OccurrenceDetails {
            shared,
            own,
            num_dropped: _,
        } = OccurrenceDetails::new(self);
        re_error::StructuredError::from_summary(self.text.clone())
            .with_details(shared)
            .with_details(own.iter().map(|lines| lines.join(", ")))
            .to_string()
    }

    /// The details of every kept occurrence, oldest first.
    fn occurrence_details(&self) -> impl Iterator<Item = &Vec<String>> {
        std::iter::chain(&self.earlier_details, std::iter::once(&self.details))
    }

    /// Whether `other` says the same thing as `self`, so it should be merged into it.
    ///
    /// Details are not compared: they tend to differ between repeats (trace ids, ids of the
    /// affected entity, …).
    fn is_repeated_by(&self, other: &Self) -> bool {
        self.level == other.level
            && self.text == other.text
            && self.permanent_dismiss_id == other.permanent_dismiss_id
            && self.link.as_ref().map(|link| &link.url) == other.link.as_ref().map(|link| &link.url)
    }

    /// Merge a repeat of this notification into it, which makes it unread and new again.
    fn merge(&mut self, repeat: Self) {
        let Self {
            level: _,
            text: _,
            details,
            earlier_details,
            count,
            link: _,
            permanent_dismiss_id: _,
            created_at,
            toast_ttl,
            is_unread: _,
            unique_id: _,
        } = repeat;

        self.earlier_details
            .push_back(std::mem::replace(&mut self.details, details));
        self.earlier_details.extend(earlier_details);
        while MAX_EARLIER_OCCURRENCES < self.earlier_details.len() {
            self.earlier_details.pop_front();
        }
        self.count += count;
        self.created_at = created_at;
        self.toast_ttl = self.toast_ttl.max(toast_ttl);
        self.is_unread = true;
    }

    pub fn with_details(mut self, details: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.details = details.into_iter().map(Into::into).collect();
        self
    }

    /// Add the structured key-value fields as details, one `key: value` per line.
    ///
    /// A field value may carry details of its own (see [`re_error::StructuredError`]); those
    /// follow it, one per line.
    pub fn with_fields(mut self, fields: Vec<(&'static str, re_log::FieldValue)>) -> Self {
        for (key, value) in fields {
            let value = re_error::StructuredError::parse(value.to_string());
            self.details.push(format!("{key}: {}", value.summary));
            self.details.extend(value.details);
        }
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

#[derive(Debug, PartialEq, Eq)]
enum TextSegment<'a> {
    Text(&'a str),
    Url(&'a str),
}

fn split_links(text: &str) -> Vec<TextSegment<'_>> {
    let mut segments = Vec::new();
    let mut plain_text_start = 0;
    let mut token_start = 0;

    for chunk in text.split_inclusive(char::is_whitespace) {
        let token = chunk.trim_end_matches(char::is_whitespace);
        let current_token_start = token_start;
        token_start += chunk.len();

        let candidate_with_suffix = token.trim_start_matches(['(', '[', '{', '<', '"', '\'']);
        let candidate = candidate_with_suffix
            .trim_end_matches(['.', ',', ':', ';', '!', '?', ')', ']', '}', '>', '"', '\'']);
        if url::Url::parse(candidate).is_err() {
            continue;
        }

        let url_start = current_token_start + (token.len() - candidate_with_suffix.len());
        if plain_text_start < url_start {
            segments.push(TextSegment::Text(&text[plain_text_start..url_start]));
        }
        segments.push(TextSegment::Url(candidate));
        plain_text_start = url_start + candidate.len();
    }

    if plain_text_start < text.len() {
        segments.push(TextSegment::Text(&text[plain_text_start..]));
    }
    segments
}

/// Show `text`, rendering embedded URLs as inline hyperlinks.
fn label_with_inline_links(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for segment in split_links(text) {
            match segment {
                TextSegment::Text(text) => {
                    ui.label(text);
                }
                TextSegment::Url(url) => {
                    // Toasts render after the viewer's hyperlink interceptor, so navigating in the
                    // same tab would replace the running Web Viewer.
                    ui.add(egui::Hyperlink::from_label_and_url(url, url).open_in_new_tab(true));
                }
            }
        }
    });
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

    next_id: u64,
}

impl NotificationUi {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            notifications: Vec::new(),
            unread_notification_level: None,
            was_open_last_frame: false,
            toasts: Toasts::new(),
            next_id: 0,
        }
    }

    pub fn unread_notification_level(&self) -> Option<NotificationLevel> {
        self.unread_notification_level
    }

    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    /// Given that the log is relevant this creates a notification
    /// based on that log.
    ///
    /// ## Special cased text
    /// - If a notifications text has details (see [`re_error::StructuredError`]), those are shown
    ///   inside a collapsible details header, one per line. Structured log fields go in there too.
    /// - An event carrying [`re_log::GUI_NOTIFICATION_FIELD`] set to `false` is dropped: it is
    ///   log-only, because whoever handles the failure reports it with more context.
    pub fn add_log(&mut self, log_msg: re_log::LogMsg) {
        let re_log::LogMsg {
            level,
            target,
            message,
            fields,
        } = log_msg;

        if !show_in_gui(&fields) {
            return;
        }

        if is_relevant(&target, level) {
            let mut message = re_error::StructuredError::parse(message);
            let mut fields = fields;

            // `#[tracing::instrument(err)]` reports the whole error as an `error` field, with no
            // message at all. That error is the message, so show it as one instead of as a field.
            if message.summary.is_empty()
                && let Some(i) = fields.iter().position(|(key, _)| *key == "error")
            {
                let (_, error) = fields.remove(i);
                message = message.concat(re_error::StructuredError::parse(error.to_string()));
            }

            let notification = Notification::new(level.into(), message.summary)
                .with_details(message.details)
                .with_fields(fields);

            self.add(notification);
        }
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.add(Notification::new(NotificationLevel::Success, text.into()));
    }

    pub fn add(&mut self, mut notification: Notification) {
        if notification.is_perma_dismissed(&self.ctx) {
            return;
        }

        if Some(notification.level) > self.unread_notification_level {
            self.unread_notification_level = Some(notification.level);
        }

        // Merge repeats, so a failure that keeps recurring shows up as one notification with a
        // count, instead of burying everything else in the panel.
        if let Some(index) = self
            .notifications
            .iter()
            .rposition(|existing| existing.is_repeated_by(&notification))
        {
            // Keep the list sorted by `created_at`, which the merge bumps.
            let mut existing = self.notifications.remove(index);
            existing.merge(notification);
            self.notifications.push(existing);
            return;
        }

        notification.unique_id = self.next_id;
        self.next_id += 1;

        self.notifications.push(notification);
    }

    /// A little bell-like button, that shows recent notifications when clicked.
    pub fn notification_toggle_button(&mut self, ui: &mut egui::Ui) {
        let popup_id = notification_panel_popup_id();

        let is_panel_visible = egui::Popup::is_id_open(ui.ctx(), popup_id);

        // The label names the unread level, since the dot that shows it is only painted.
        let label = match self.unread_notification_level {
            Some(level) => format!("Notification toggle, unread {}", level.name()),
            None => "Notification toggle".to_owned(),
        };
        let button_response = ui.medium_icon_toggle_button(
            &icons::NOTIFICATION,
            label,
            &mut is_panel_visible.clone(),
        );

        if let Some(level) = self.unread_notification_level {
            let color = match level {
                NotificationLevel::Tip | NotificationLevel::Info => ui.tokens().attention_dot_color,
                _ => level.color(ui),
            };
            ui.paint_button_dot(button_response.rect, color);
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
            if notifications.is_empty() {
                ui.strong("Notifications");
            } else {
                ui.strong(format!("Notifications ({})", notifications.len()));
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
            id: egui::Id::unique("__toasts"),
        }
    }

    /// Shows and updates all toasts
    fn show(&self, egui_ctx: &egui::Context, notifications: &mut [Notification]) {
        let dt = Duration::try_from_secs_f32(egui_ctx.input(|i| i.unstable_dt))
            .unwrap_or(std::time::Duration::from_millis(100));

        let mut offset = egui::vec2(-8.0, 32.0);

        let mut first_nonzero_ttl = None;

        // Keyed by `unique_id`, not position: a repeat moves its notification to the end, and
        // must not reset the state (e.g. open details) of the toasts around it.
        for notification in notifications
            .iter_mut()
            .filter(|n| n.toast_ttl > Duration::ZERO)
        {
            first_nonzero_ttl.get_or_insert(notification.toast_ttl);

            let response = egui::Area::new(self.id.with(notification.unique_id))
                .anchor(egui::Align2::RIGHT_TOP, offset)
                .order(egui::Order::Foreground)
                .interactable(true)
                .movable(false)
                .role(egui::Role::Alert)
                .accessible_name("Notification")
                .show(egui_ctx, |ui| {
                    show_notification(ui, notification, DisplayMode::Toast).0
                });
            let closed = response.inner.is_some();
            let response = response.response;

            // Reading the details is what a toast is for; don't yank it away mid-read.
            let details_are_open = 0.0
                < egui_ctx.data(|data| {
                    details_openness(data, notification.unique_id, DisplayMode::Toast)
                });

            if !response.hovered()
                && !details_are_open
                && !egui_ctx.rect_contains_pointer(response.layer_id, response.interact_rect)
            {
                notification.toast_ttl = notification.toast_ttl.saturating_sub(dt);
            }

            let response = response.on_hover_text("Click to close and copy contents");

            if closed {
                notification.toast_ttl = Duration::ZERO;
            } else if response.clicked() {
                if let Some(link) = &notification.link {
                    egui_ctx.open_url(egui::OpenUrl::new_tab(link.url.clone()));
                } else {
                    egui_ctx.copy_text(notification.copy_text());
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

/// Where a notification is being shown, which decides what it may do with the space it has.
#[derive(Clone, Copy)]
enum DisplayMode {
    /// In the notification panel, where it stays until dismissed.
    ///
    /// It is stuck with the width of the panel, and shows its age and a dismiss button.
    Panel,

    /// As a toast floating over the viewport, until its time to live runs out.
    ///
    /// It is free-floating, so it may grow sideways to fit its expanded details.
    Toast,
}

/// Where the last shown openness of a notification's details section is kept, so that the toast
/// can widen for it and stay up while it is open.
///
/// The same notification can be shown in both modes at once, each with its own details section,
/// so the two must not share a key.
fn details_openness_id(unique_id: u64, mode: DisplayMode) -> egui::Id {
    let mode = match mode {
        DisplayMode::Panel => "panel",
        DisplayMode::Toast => "toast",
    };
    egui::Id::unique(("notification_details_openness", unique_id, mode))
}

/// How open the details section of this notification is, from 0 (collapsed) to 1 (expanded).
///
/// This is what the previous frame drew, since the details are shown after the width is decided.
fn details_openness(data: &egui::util::IdTypeMap, unique_id: u64, mode: DisplayMode) -> f32 {
    data.get_temp::<f32>(details_openness_id(unique_id, mode))
        .unwrap_or(0.0)
}

/// The width of the text column of a notification, with its details collapsed.
const NARROW_WIDTH: f32 = 270.0;

/// The width of the text column of a toast with its details expanded.
const WIDE_WIDTH: f32 = 420.0;

/// The height of the details section, beyond which it scrolls.
const MAX_DETAILS_HEIGHT: f32 = 400.0;

/// How many occurrences of a repeated notification show their details, newest first.
///
/// More would make a toast taller than the viewport. Copying the notification copies them all.
const MAX_SHOWN_OCCURRENCES: usize = 10;

/// Room to leave around a toast, so a wide one doesn't touch the edges of the viewport.
const TOAST_MARGIN: f32 = 32.0;

fn show_notification(
    ui: &mut egui::Ui,
    notification: &Notification,
    mode: DisplayMode,
) -> (Option<NotificationReaction>, egui::Response) {
    let Notification {
        level,
        text,
        details: _,
        earlier_details: _,
        count,
        link,
        permanent_dismiss_id,
        created_at,
        toast_ttl: _,
        is_unread,
        unique_id,
    } = notification;

    // A toast is free-floating, so it may grow sideways to fit expanded details.
    // A notification in the panel is stuck with the width of the panel.
    let openness_id = details_openness_id(*unique_id, mode);
    let openness = ui.data(|data| details_openness(data, *unique_id, mode));
    let width = match mode {
        DisplayMode::Toast => {
            let max_width =
                (ui.ctx().viewport_rect().width() - 2.0 * TOAST_MARGIN).at_least(NARROW_WIDTH);
            if 0.0 < openness && openness < 1.0 {
                ui.ctx().request_repaint();
            }
            egui::lerp(NARROW_WIDTH..=WIDE_WIDTH.min(max_width), openness)
        }
        DisplayMode::Panel => NARROW_WIDTH,
    };

    ui.push_id(unique_id, |ui| {
        let background_color = match mode {
            DisplayMode::Toast => ui.tokens().notification_background_color,
            DisplayMode::Panel if *is_unread => ui.tokens().notification_background_color,
            DisplayMode::Panel => ui.tokens().notification_panel_background_color,
        };

        let mut reaction = None;

        let response = egui::Frame::window(ui.style())
            .corner_radius(match mode {
                DisplayMode::Panel => ui.tokens().small_corner_radius(),
                DisplayMode::Toast => ui.tokens().window_corner_radius(),
            })
            .inner_margin(10.0)
            .fill(background_color)
            .shadow(egui::Shadow::NONE)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.horizontal_top(|ui| {
                        ui.add(level.image(ui));

                        ui.vertical(|ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            ui.set_width(width);
                            if !text.is_empty() {
                                label_with_inline_links(ui, text);
                            }
                            if 1 < *count {
                                ui.label(egui::RichText::new(format!("{count} times")).weak());
                            }

                            let details = OccurrenceDetails::new(notification);
                            if !details.is_empty() {
                                let max_height = (ui.ctx().content_rect().height() / 2.0)
                                    .at_most(MAX_DETAILS_HEIGHT);
                                let response = ui.collapsing_header("Details", false, |ui| {
                                    // The explicit minimum is needed: this is laid out in a row,
                                    // which leaves the scroll area almost no height of its own.
                                    egui::ScrollArea::vertical()
                                        .max_height(max_height)
                                        .min_scrolled_height(max_height)
                                        .show(ui, |ui| details.ui(ui));
                                });

                                // Read one frame later, to widen the toast for the details.
                                ui.data_mut(|data| {
                                    data.insert_temp(openness_id, response.openness);
                                });
                                if response.openness != openness {
                                    ui.ctx().request_repaint();
                                }
                            } else if 0.0 < openness {
                                // The details were dropped while open; an open section that is no
                                // longer shown would keep the toast up forever.
                                ui.data_mut(|data| data.remove::<f32>(openness_id));
                            }
                        });

                        ui.add_space(4.0);
                        match mode {
                            DisplayMode::Panel => notification_age_label(ui, *created_at),
                            DisplayMode::Toast => {
                                // Open details keep a toast up, so it needs a way to go away.
                                if ui.small_icon_button(&icons::CLOSE, "Close").clicked() {
                                    reaction = Some(NotificationReaction::Dismissed);
                                }
                            }
                        }
                    });

                    let show_dismiss = match mode {
                        DisplayMode::Panel => true,
                        DisplayMode::Toast => false,
                    };
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
    })
    .inner
}

/// The details of a notification, with the lines that all its occurrences share shown once.
struct OccurrenceDetails<'a> {
    /// Lines that every kept occurrence has, in the order of the oldest one.
    shared: Vec<&'a str>,

    /// The rest of the lines of each kept occurrence, newest first.
    ///
    /// Empty for a notification that happened only once, or whose occurrences all said the same.
    own: Vec<Vec<&'a str>>,

    /// Occurrences that are counted, but whose details are no longer kept.
    num_dropped: usize,
}

impl<'a> OccurrenceDetails<'a> {
    fn new(notification: &'a Notification) -> Self {
        let occurrences: Vec<&Vec<String>> = notification.occurrence_details().collect();
        let first = occurrences
            .first()
            .map(|lines| lines.as_slice())
            .unwrap_or_default();
        let shared: Vec<&str> = first
            .iter()
            .filter(|line| occurrences.iter().all(|lines| lines.contains(line)))
            .map(String::as_str)
            .collect();
        let own: Vec<Vec<&str>> = occurrences
            .iter()
            .rev()
            .map(|lines| {
                lines
                    .iter()
                    .map(String::as_str)
                    .filter(|line| !shared.contains(line))
                    .collect::<Vec<_>>()
            })
            .filter(|lines| !lines.is_empty())
            .collect();
        Self {
            shared,
            own,
            num_dropped: notification.count.saturating_sub(occurrences.len()),
        }
    }

    fn is_empty(&self) -> bool {
        let Self {
            shared,
            own,
            num_dropped: _,
        } = self;
        shared.is_empty() && own.is_empty()
    }

    fn ui(&self, ui: &mut egui::Ui) {
        let Self {
            shared,
            own,
            num_dropped,
        } = self;
        let detail_label = |ui: &mut egui::Ui, line: &str| {
            ui.label(egui::RichText::new(line).monospace().weak());
        };
        for line in shared {
            detail_label(ui, line);
        }
        for (i, lines) in own.iter().take(MAX_SHOWN_OCCURRENCES).enumerate() {
            if 0 < i || !shared.is_empty() {
                ui.separator();
            }
            for line in lines {
                detail_label(ui, line);
            }
        }
        let num_hidden = own.len().saturating_sub(MAX_SHOWN_OCCURRENCES) + num_dropped;
        if 0 < num_hidden && !own.is_empty() {
            ui.separator();
            ui.weak(format!("…and {num_hidden} earlier"));
        }
    }
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
        ui.request_repaint_after(Duration::from_mins(1));

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field becomes a `key: value` detail, and a field that is an error with details of
    /// its own brings those along, right after it.
    #[test]
    fn test_fields_become_details() {
        use re_log::FieldValue;

        let notification = Notification::new(NotificationLevel::Error, "Failed to connect")
            .with_fields(vec![
                ("user_name", FieldValue::String("bob".to_owned())),
                ("num_attempts", FieldValue::I64(42)),
                (
                    "error",
                    FieldValue::Error("it failed\n- the fine print".to_owned()),
                ),
            ]);

        assert_eq!(
            notification.details,
            [
                "user_name: bob",
                "num_attempts: 42",
                "error: it failed",
                "the fine print",
            ]
        );
    }

    #[test]
    fn test_split_links() {
        use TextSegment::{Text, Url};

        assert_eq!(
            split_links("See https://rerun.invalid/docs and (http://example.invalid/help)."),
            vec![
                Text("See "),
                Url("https://rerun.invalid/docs"),
                Text(" and ("),
                Url("http://example.invalid/help"),
                Text(")."),
            ]
        );
        assert_eq!(
            split_links("Not a link: rerun.invalid/docs; this is: mailto:help@example.invalid"),
            vec![
                Text("Not a link: rerun.invalid/docs; this is: "),
                Url("mailto:help@example.invalid"),
            ]
        );
    }

    /// `#[tracing::instrument(err)]` logs the whole error as an `error` field and no message.
    /// That error is the message, so it must not end up as a dim `error: …` field.
    #[test]
    fn test_lone_error_field_becomes_the_message() {
        let mut ui = NotificationUi::new(egui::Context::default());

        ui.add_log(re_log::LogMsg {
            level: re_log::Level::ERROR,
            target: "re_ui".to_owned(),
            message: String::new(),
            fields: vec![(
                "error",
                re_log::FieldValue::Error("it failed\n- the fine print".to_owned()),
            )],
        });

        let notification = ui
            .notifications()
            .first()
            .expect("the log should have become a notification");
        assert_eq!(notification.text(), "it failed");
        assert_eq!(notification.details, ["the fine print"]);
    }

    fn notification(level: NotificationLevel, text: &str, detail: &str) -> Notification {
        Notification::new(level, text).with_details([detail])
    }

    /// Repeats merge into one notification that moves to the end, keeping every occurrence's
    /// details; anything that differs in level or text stays separate.
    #[test]
    fn test_repeats_are_merged() {
        let mut ui = NotificationUi::new(egui::Context::default());
        ui.add(notification(NotificationLevel::Error, "Timed out", "a"));
        ui.add(notification(NotificationLevel::Error, "Other", "b"));
        ui.add(notification(NotificationLevel::Warning, "Timed out", "c"));
        ui.add(notification(NotificationLevel::Error, "Timed out", "d"));

        let summary: Vec<_> = ui
            .notifications()
            .iter()
            .map(|n| (n.level(), n.text(), n.count))
            .collect();
        assert_eq!(
            summary,
            [
                (NotificationLevel::Error, "Other", 1),
                (NotificationLevel::Warning, "Timed out", 1),
                (NotificationLevel::Error, "Timed out", 2),
            ]
        );
        let merged = &ui.notifications()[2];
        assert_eq!(merged.copy_text(), "Timed out\n- d\n- a");
    }

    /// Lines every occurrence has are shown once; the rest are grouped newest first, and
    /// occurrences whose details were dropped are still counted.
    #[test]
    fn test_occurrence_details() {
        let mut merged = Notification::new(NotificationLevel::Error, "Timed out")
            .with_details(["server: x", "trace-id: 0"]);
        for i in 1..=MAX_EARLIER_OCCURRENCES + 2 {
            merged.merge(
                Notification::new(NotificationLevel::Error, "Timed out")
                    .with_details(["server: x".to_owned(), format!("trace-id: {i}")]),
            );
        }

        let details = OccurrenceDetails::new(&merged);
        assert_eq!(merged.count, MAX_EARLIER_OCCURRENCES + 3);
        assert_eq!(details.shared, ["server: x"]);
        assert_eq!(details.own.len(), MAX_EARLIER_OCCURRENCES + 1);
        assert_eq!(
            details.own.first(),
            Some(&vec![
                format!("trace-id: {}", MAX_EARLIER_OCCURRENCES + 2).as_str()
            ])
        );
        assert_eq!(details.num_dropped, 2);
    }

    /// A log-only event must not reach the panel, or the user sees one failure twice: once from
    /// where it was noticed, once from where it was handled.
    #[test]
    fn test_log_only_event_is_not_notified() {
        let mut ui = NotificationUi::new(egui::Context::default());

        ui.add_log(re_log::LogMsg {
            level: re_log::Level::WARN,
            target: "re_ui".to_owned(),
            message: "gRPC streaming response failed".to_owned(),
            fields: vec![
                (
                    re_log::GUI_NOTIFICATION_FIELD,
                    re_log::FieldValue::Bool(false),
                ),
                (
                    "endpoint",
                    re_log::FieldValue::String("/FetchChunks".to_owned()),
                ),
            ],
        });

        assert!(ui.notifications().is_empty());
    }
}
