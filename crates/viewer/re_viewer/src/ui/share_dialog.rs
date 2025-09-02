use egui::NumExt as _;
use re_ui::{UiExt as _, icons};
use re_viewer_context::{DisplayMode, StoreHub};

use crate::open_url::ViewerOpenUrl;

#[derive(Default)]
pub struct ShareDialog {
    url: Option<ViewerOpenUrl>,
    web_viewer_url: bool,
}

impl ShareDialog {
    /// Button that opens the share popup.
    pub fn ui(&mut self, ui: &mut egui::Ui, store_hub: &StoreHub, display_mode: &DisplayMode) {
        re_tracing::profile_function!();

        let popup_id = egui::Id::new("share_dialog_popup");
        let is_panel_visible = egui::Popup::is_id_open(ui.ctx(), popup_id);

        let url_for_current_screen =
            ViewerOpenUrl::from_display_mode(store_hub, display_mode.clone());
        let enable_share_button = !is_panel_visible && url_for_current_screen.is_ok();

        let share_button_resp = ui
            .add_enabled_ui(enable_share_button, |ui| ui.button("Share"))
            .inner;

        let button_response = match url_for_current_screen {
            Err(err) => share_button_resp
                .on_disabled_hover_text(format!("Cannot create share URL: {}", err)),
            Ok(url) => {
                if share_button_resp.clicked() {
                    self.url = Some(url);
                }
                share_button_resp
            }
        };

        if let Some(url) = self.url.as_mut() {
            egui::Popup::from_toggle_button_response(&button_response)
                .id(popup_id)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .frame(ui.tokens().popup_frame(ui.style()))
                .align(egui::RectAlign::BOTTOM_END)
                .show(|ui| {
                    popup_contents(ui, url, &mut self.web_viewer_url);
                });
        }
    }
}

fn popup_contents(
    ui: &mut egui::Ui,
    url: &mut ViewerOpenUrl,
    create_web_viewer_url: &mut bool,
    //notifications: &mut NotificationUi,
) {
    let panel_width = 356.0;
    let panel_max_height = (ui.ctx().screen_rect().height() - 100.0)
        .at_least(0.0)
        .at_most(640.0);

    ui.set_width(panel_width);
    ui.set_max_height(panel_max_height);

    ui.horizontal_top(|ui| {
        ui.strong("Share");
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            if ui.small_icon_button(&icons::CLOSE, "Close").clicked() {
                ui.close();
            }
        });
    });

    // Style URL box like a test edit.
    let url_string = url.sharable_url(None).unwrap_or_else(|_err| {
        // TODO: better error handling. When would this happen?
        "<no valid share URL>".to_owned()
    });

    ui.add_enabled(false, |ui: &mut egui::Ui| {
        let mut url = url_string.clone();
        egui::TextEdit::singleline(&mut url)
            .desired_width(f32::INFINITY)
            .show(ui)
            .response
    });

    let copy_link_response = ui.add(
        egui::Button::new((&icons::EXTERNAL_LINK, "Copy link"))
            .min_size(egui::vec2(f32::INFINITY, 0.0)),
    );
    if copy_link_response.clicked() {
        ui.ctx().copy_text(url_string.clone());

        // TODO: why is this not globally available?
        //notifications.success(format!("Copied {url_string:?} to clipboard"));
    }

    ui.checkbox(create_web_viewer_url, "Web viewer URL")
        .on_hover_text("Create a link that can be opened directly in the browser.");

    // TODO: allow only for supported link types and present right tooltip on disabled.
    ui.collapsing("Customize timing", |ui| {
        ui.label("I owe you timing settings");
    });
}
