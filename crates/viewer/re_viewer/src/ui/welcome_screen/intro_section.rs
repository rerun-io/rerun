use eframe::epaint::Margin;
use egui::{Button, Frame, RichText, TextStyle, Theme, Ui};
use re_ui::egui_ext::card_layout::{CardLayout, CardLayoutItem};
use re_ui::{ReButtonExt as _, UICommand, UICommandSender as _, UiExt as _, design_tokens_of};
use re_uri::Origin;
use re_viewer_context::{
    EditRedapServerModalCommand, GlobalContext, Item, SystemCommand, SystemCommandSender as _,
};

pub enum LoginState {
    NoAuth,
    Auth { email: Option<String> },
}

pub struct CloudState {
    pub has_server: Option<Origin>,
    pub login: LoginState,
}

pub enum IntroItem<'a> {
    DocItem {
        title: &'static str,
        url: &'static str,
        body: &'static str,
    },
    CloudLoginItem(&'a CloudState),
}

impl<'a> IntroItem<'a> {
    fn items(login_state: &'a CloudState) -> Vec<Self> {
        vec![
            IntroItem::DocItem {
                title: "Send data in",
                url: "https://rerun.io/docs/getting-started/data-in",
                body: "Send data to Rerun from your running applications or existing files.",
            },
            IntroItem::DocItem {
                title: "Explore data",
                url: "https://rerun.io/docs/getting-started/configure-the-viewer",
                body: "Familiarize yourself with the basics of using the Rerun Viewer.",
            },
            IntroItem::DocItem {
                title: "Query data out",
                url: "https://rerun.io/docs/getting-started/data-out",
                body: "Perform analysis and send back the results to the original recording.",
            },
            IntroItem::CloudLoginItem(login_state),
        ]
    }

    fn frame(&self, ui: &Ui) -> Frame {
        let opposite_theme = match ui.theme() {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        };
        let opposite_tokens = design_tokens_of(opposite_theme);
        let tokens = ui.tokens();
        let frame = Frame::new()
            .inner_margin(Margin::same(16))
            .corner_radius(8)
            .stroke(tokens.native_frame_stroke);
        match self {
            IntroItem::DocItem { .. } => frame,
            IntroItem::CloudLoginItem(_) => frame.fill(opposite_tokens.panel_bg_color),
        }
    }

    fn card_item(&self, ui: &Ui) -> CardLayoutItem {
        let frame = self.frame(ui);
        let min_width = match &self {
            IntroItem::DocItem { .. } => 200.0,
            IntroItem::CloudLoginItem(_) => 400.0,
        };
        CardLayoutItem { frame, min_width }
    }

    fn show(&self, ui: &mut Ui, ctx: &GlobalContext<'_>) {
        let label_size = 13.0;
        ui.vertical(|ui| match self {
            IntroItem::DocItem { title, url, body } => {
                egui::Sides::new().shrink_left().show(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

                    ui.heading(RichText::new(*title).strong());
                }, |ui| {
                    ui.re_hyperlink("Docs", *url, true);
                });
                ui.label(RichText::new(*body).size(label_size));
            }
            IntroItem::CloudLoginItem(login_state) => {
                let opposite_theme = match ui.theme() {
                    Theme::Dark => Theme::Light,
                    Theme::Light => Theme::Dark,
                };
                ui.set_style(ui.ctx().style_of(opposite_theme));

                ui.heading(RichText::new("Rerun Cloud").strong());

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.style_mut().text_styles.get_mut(&TextStyle::Body).expect("Should always have body text style").size = label_size;
                    ui.label(
                        "Iterate faster on robotics learning with unified infrastructure. Interested? Read more "
                    );
                    ui.hyperlink_to("here", "https://rerun.io/");
                    ui.label(" or ");
                    ui.hyperlink_to("book a demo", "https://calendly.com/d/ctht-4kp-qnt/rerun-demo-meeting");
                    ui.label(".");
                });

                match login_state {
                    CloudState { has_server: None, login: LoginState::NoAuth } => {
                        if ui.primary_button("Add server and login").clicked() {
                            ctx.command_sender.send_ui(UICommand::AddRedapServer);
                        }
                    }
                    CloudState { has_server: None, login } => {
                        ui.horizontal_wrapped(|ui| {
                            if ui.primary_button("Add server").clicked() {
                                ctx.command_sender.send_ui(UICommand::AddRedapServer);
                            }
                            if let LoginState::Auth { email: Some(email) } = login {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.weak("logged in as ");
                                ui.strong(email);
                            }
                        });
                    }
                    CloudState { has_server: Some(origin), login: LoginState::NoAuth } => {
                    ui.horizontal_wrapped(|ui| {
                        if ui.primary_button("Add credentials").clicked() {
                            ctx.command_sender.send_system(SystemCommand::EditRedapServerModal(EditRedapServerModalCommand::new(origin.clone())));
                        }
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.weak("for address ");
                        ui.strong(format!("{}", &origin.host));
                        });
                    }
                    CloudState { has_server: Some(origin), login: LoginState::Auth { .. } } => {
                        if ui.primary_button("Explore your data").clicked() {
                            ctx.command_sender.send_system(SystemCommand::set_selection(Item::RedapServer(origin.clone())));
                        }
                    }
                }
            }
        });
    }
}

pub fn intro_section(ui: &mut egui::Ui, ctx: &GlobalContext<'_>, login_state: &CloudState) {
    let items = IntroItem::items(login_state);

    ui.add_space(32.0);

    if let Some(auth) = ctx.auth_context {
        ui.strong(RichText::new(format!("Hi, {}!", &auth.email)).size(15.0));

        if ui.add(Button::new("Log out").secondary().small()).clicked() {
            ctx.command_sender.send_system(SystemCommand::Logout);
        }

        ui.add_space(32.0);
    }

    CardLayout::new(items.iter().map(|item| item.card_item(ui)).collect()).show(ui, |ui, index| {
        let item = &items[index];
        item.show(ui, ctx);
    });
}
