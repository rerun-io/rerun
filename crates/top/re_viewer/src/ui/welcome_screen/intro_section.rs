use egui::{Button, Frame, RichText, Ui};
use re_ui::egui_ext::card_layout::CardLayout;
use re_ui::{ButtonExt as _, DesignTokens, Icon, UICommand, UICommandSender as _, UiExt as _};
use re_uri::Origin;
use re_viewer_context::{
    AppContext, EditRedapServerModalCommand, Item, SystemCommand, SystemCommandSender as _,
};

use super::entrance_animation::{EntranceAnimation, EntranceAnimationItem};

const INTRO_CARD_DELAY: f32 = 0.18;
const INTRO_CARD_ANIMATION: EntranceAnimationItem = EntranceAnimationItem::new(0.34, 0.08);
const INTRO_CARD_MIN_WIDTH: f32 = 199.0;

const SEND_DATA_ICON: Icon = Icon::new(
    "welcome_send_data.svg",
    include_bytes!("../../../data/welcome_send_data.svg"),
);
const EXPLORE_DATA_ICON: Icon = Icon::new(
    "welcome_explore_data.svg",
    include_bytes!("../../../data/welcome_explore_data.svg"),
);
const QUERY_DATA_ICON: Icon = Icon::new(
    "welcome_query_data.svg",
    include_bytes!("../../../data/welcome_query_data.svg"),
);
const RERUN_HUB_ICON: Icon = Icon::new(
    "welcome_rerun_hub.svg",
    include_bytes!("../../../data/welcome_rerun_hub.svg"),
);

pub enum LoginState {
    NoAuth,
    Auth { email: Option<String> },
}

pub struct CloudState {
    pub has_server: Option<Origin>,
    pub login: LoginState,
}

#[cfg(feature = "analytics")] // these are currently only used when analytics is enabled
impl CloudState {
    fn is_logged_in(&self) -> bool {
        matches!(self.login, LoginState::Auth { .. })
    }

    fn has_server(&self) -> bool {
        self.has_server.is_some()
    }
}

pub enum IntroItem {
    DocItem {
        icon: &'static Icon,
        title: &'static str,
        url: &'static str,
        body: &'static str,
    },
    CloudLoginItem,
}

impl IntroItem {
    fn items(login_enabled: bool) -> Vec<Self> {
        let mut items = vec![
            Self::DocItem {
                icon: &SEND_DATA_ICON,
                title: "Send data in",
                url: "https://rerun.io/docs/getting-started/data-in",
                body: "Ingest multi-rate, multimodal data from robot logs, sensors, simulation, or video.",
            },
            Self::DocItem {
                icon: &EXPLORE_DATA_ICON,
                title: "Explore data",
                url: "https://rerun.io/docs/getting-started/configure-the-viewer",
                body: "Visualize and explore multi-rate, multimodal data across every stage of the pipeline.",
            },
            Self::DocItem {
                icon: &QUERY_DATA_ICON,
                title: "Query data out",
                url: "https://rerun.io/docs/getting-started/data-out",
                body: "Query raw, intermediate, and derived data with dataframes or SQL, and stream to training.",
            },
        ];
        if login_enabled {
            items.push(Self::CloudLoginItem);
        }
        items
    }

    fn icon(&self) -> &'static Icon {
        match self {
            Self::DocItem { icon, .. } => icon,
            Self::CloudLoginItem => &RERUN_HUB_ICON,
        }
    }

    fn animation_id(&self) -> &'static str {
        match self {
            Self::DocItem { title, .. } => title,
            Self::CloudLoginItem => "rerun_hub",
        }
    }

    fn header(&self, ui: &mut Ui, title: &str) {
        ui.add(self.icon().as_image().tint(ui.tokens().text_strong));
        ui.add_space(18.0);
        ui.add(
            egui::Label::new(
                RichText::new(title)
                    .strong()
                    .text_style(DesignTokens::heading_3())
                    .color(ui.tokens().text_strong),
            )
            .wrap(),
        );
    }

    fn show(&self, ui: &mut Ui, ctx: &AppContext<'_>, cloud_state: &CloudState) {
        re_tracing::profile_function!();

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            match self {
                Self::DocItem {
                    title, url, body, ..
                } => {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    self.header(ui, title);
                    ui.add_space(11.0);
                    ui.label(
                        RichText::new(*body)
                            .text_style(DesignTokens::body())
                            .color(ui.tokens().text_default),
                    );
                    ui.add_space(11.0);

                    let response = ui.add(Button::new("Read docs").outlined().size_tiny());
                    if response.clicked_with_open_in_background() || response.clicked() {
                        ui.open_url(egui::OpenUrl::new_tab(*url));
                        #[cfg(feature = "analytics")]
                        re_analytics::record(|| re_analytics::event::WelcomeScreenNavigation {
                            card_type: "docs".to_owned(),
                            destination: (*url).to_owned(),
                            cta_cloud: false,
                            is_logged_in: cloud_state.is_logged_in(),
                            has_server: cloud_state.has_server(),
                        });
                    }
                }
                Self::CloudLoginItem => {
                    self.header(ui, "Rerun Hub");
                    ui.add_space(11.0);
                    ui.label(
                        RichText::new(
                            "The production backend for the Rerun data layer, for anyone shipping physical AI at scale.",
                        )
                        .text_style(DesignTokens::body())
                        .color(ui.tokens().text_default),
                    );
                    ui.add_space(11.0);

                    let connect = ui.add(Button::new("Connect").outlined().size_tiny());
                    let connect = match &cloud_state.login {
                        LoginState::Auth { email: Some(email) } => {
                            connect.on_hover_text(format!("Logged in as {email}"))
                        }
                        LoginState::NoAuth | LoginState::Auth { email: None } => connect,
                    };

                    if connect.clicked() {
                        #[cfg(feature = "analytics")]
                        re_analytics::record(|| re_analytics::event::WelcomeScreenNavigation {
                            card_type: "redap".to_owned(),
                            destination: String::new(),
                            cta_cloud: true,
                            is_logged_in: cloud_state.is_logged_in(),
                            has_server: cloud_state.has_server(),
                        });

                        match cloud_state {
                            CloudState {
                                has_server: None, ..
                            } => {
                                ctx.command_sender.send_ui(UICommand::OpenAddServerDialog);
                            }
                            CloudState {
                                has_server: Some(origin),
                                login: LoginState::NoAuth,
                            } => {
                                ctx.command_sender.send_system(
                                    SystemCommand::EditRedapServerModal(
                                        EditRedapServerModalCommand::new(origin.clone()),
                                    ),
                                );
                            }
                            CloudState {
                                has_server: Some(origin),
                                login: LoginState::Auth { .. },
                            } => {
                                ctx.command_sender.send_system(SystemCommand::set_selection(
                                    Item::RedapServer(origin.clone()),
                                ));
                            }
                        }
                    }
                }
            }
        });
    }
}

pub fn intro_section(
    ui: &mut egui::Ui,
    ctx: &AppContext<'_>,
    cloud_state: &CloudState,
    animation: &mut EntranceAnimation,
) {
    re_tracing::profile_function!();
    let items = IntroItem::items(ctx.login_enabled);

    ui.add_space(37.0);

    if let Some(auth) = ctx.auth_context {
        ui.label(
            RichText::new(format!("Hi, {}!", auth.email))
                .strong()
                .text_style(DesignTokens::heading_3()),
        );

        if ui
            .add(Button::new("Log out").secondary().size_small())
            .clicked()
        {
            ctx.command_sender.send_system(SystemCommand::Logout);
        }

        ui.add_space(32.0);
    }

    animation.add_delay(INTRO_CARD_DELAY);
    let animation_id = ui.scope_id().with("intro_cards");
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 16.0);
        let min_width = intro_card_min_width(
            ui.available_width(),
            items.len(),
            ui.spacing().item_spacing.x,
        );
        CardLayout::uniform(items.len(), min_width, Frame::NONE)
            .all_rows_use_available_width(false)
            .show(ui, |ui, index, _card_hovered| {
                animation.show(
                    ui,
                    animation_id.with(items[index].animation_id()),
                    INTRO_CARD_ANIMATION,
                    |ui| items[index].show(ui, ctx, cloud_state),
                );
            });
    });
}

fn intro_card_min_width(available_width: f32, item_count: usize, spacing: f32) -> f32 {
    if item_count != 4 {
        return INTRO_CARD_MIN_WIDTH;
    }

    if intro_card_column_count(available_width, spacing) == 2 {
        (available_width - 2.0 * spacing) / 2.0
    } else {
        INTRO_CARD_MIN_WIDTH
    }
}

fn intro_card_column_count(available_width: f32, spacing: f32) -> usize {
    let four_column_width = 4.0 * (INTRO_CARD_MIN_WIDTH + spacing);
    let two_column_width = 2.0 * (INTRO_CARD_MIN_WIDTH + spacing);

    if available_width >= four_column_width {
        4
    } else if available_width >= two_column_width {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_intro_cards_skip_the_three_column_layout() {
        assert_eq!(intro_card_column_count(828.0, 8.0), 4);
        assert_eq!(intro_card_column_count(827.0, 8.0), 2);
        assert_eq!(intro_card_column_count(414.0, 8.0), 2);
        assert_eq!(intro_card_column_count(413.0, 8.0), 1);
    }
}
