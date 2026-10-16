//! The Rerun menu used on platforms without a native application menu.

use egui::containers::menu::{MenuButton, MenuConfig};
use re_ui::{CommandKind, UICommand, UiExt as _, icons};
use re_viewer_context::{ActiveStoreContext, CommandSender, StoreHub};

use crate::App;
use crate::command_catalog::{CommandContext, MenuGroup, MenuItem, dispatch, primary_shortcut};

impl App {
    pub fn rerun_menu_button_ui(
        &self,
        store_hub: &StoreHub,
        store_context: Option<&ActiveStoreContext<'_>>,
        ui: &mut egui::Ui,
    ) {
        let menu = if cfg!(target_arch = "wasm32") || self.app_env().is_test() {
            let icon_tint = ui.tokens().strong_fg_color;
            let image = icons::RERUN_WORDMARK
                .as_image()
                .max_height(12.0)
                .tint(icon_tint)
                .alt_text("Menu");
            MenuButton::new((image, icons::DROPDOWN_ARROW.as_image().tint(icon_tint)))
        } else {
            MenuButton::new("Menu")
        };

        menu.config(MenuConfig::new().style(re_ui::menu::menu_style()))
            .ui(ui, |ui| {
                let commands = self.command_context(store_hub, store_context);
                let command_button = |ui: &mut egui::Ui, command| {
                    command_button_ui(
                        ui,
                        &commands,
                        &self.command_sender,
                        CommandKind::Ui(command),
                    );
                };
                let submenu = |ui: &mut egui::Ui, group| {
                    submenu_ui(ui, &commands, &self.command_sender, group);
                };

                command_button(ui, UICommand::About);
                command_button(ui, UICommand::OpenSettings);
                ui.separator();

                for group in MenuGroup::ALL {
                    if group == MenuGroup::Application {
                        continue;
                    }
                    if group == MenuGroup::Help {
                        ui.separator();
                    }
                    submenu(ui, group);
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.separator();
                    command_button(ui, UICommand::Quit);
                }
            });
    }
}

fn submenu_ui(
    ui: &mut egui::Ui,
    commands: &CommandContext,
    sender: &CommandSender,
    group: MenuGroup,
) {
    let mut items = commands.visible_items(group).peekable();
    if items.peek().is_none() {
        return;
    }

    ui.menu_button(group.text(), |ui| {
        for (_, item) in items {
            match item {
                MenuItem::Command(kind) => command_button_ui(ui, commands, sender, kind),
                MenuItem::Separator => {
                    ui.separator();
                }
            }
        }
    });
}

fn command_button_ui(
    ui: &mut egui::Ui,
    commands: &CommandContext,
    sender: &CommandSender,
    kind: CommandKind,
) {
    if !commands.is_visible(kind) {
        return;
    }

    let resolved = commands.resolve(kind);
    let mut button = egui::Button::new(kind.text());
    if let Some(shortcut) = primary_shortcut(kind, ui.os()) {
        button = button.shortcut_text(re_ui::format_shortcut(ui.ctx(), &shortcut));
    }
    if ui
        .add_enabled(resolved.is_some(), button)
        .on_hover_text(kind.tooltip())
        .clicked()
        && let Some(command) = resolved
    {
        dispatch(command, sender);
        ui.close();
    }
}
