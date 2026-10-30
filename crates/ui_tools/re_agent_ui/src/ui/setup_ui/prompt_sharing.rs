//! The opt-in to sharing redacted prompts with Rerun.

use egui::RichText;

/// What prompt sharing actually does, behind the checkbox and the "Details" link.
const ANALYTICS_DETAILS: &str = "Prompts and the agent's answers go to Rerun only after a second \
     session of your own agent has removed names, identifiers, paths, and secrets, which spends \
     your own quota.\n\
     Analytics are processed on Rerun's behalf by PostHog, on servers in the EU. They are not \
     sold, and not shared with anyone else.\n\
     Content-free usage statistics are sent whether or not this is on.";

/// How to turn off all analytics, appended to [`ANALYTICS_DETAILS`]. Only native has a switch for it.
const ANALYTICS_OPT_OUT: &str = cfg_select! {
    target_arch = "wasm32" => "",
    _ => " `rerun analytics disable` turns off all analytics.",
};

/// The width the analytics details wrap at, in both the menu and the tooltip.
const ANALYTICS_DETAILS_WIDTH: f32 = 340.0;

/// The prompt-sharing opt-in, and one line on why it helps.
///
/// The particulars stay in the tooltip that the checkbox and "Details" share.
pub fn prompt_analytics_ui(ui: &mut egui::Ui, share_redacted_prompts: &mut bool) {
    ui.scope(|ui| {
        // The explanation belongs to the checkbox above it, closer than the panel's rhythm.
        ui.spacing_mut().item_spacing.y = 2.0;

        ui.checkbox(
            share_redacted_prompts,
            "Share anonymized prompts with Rerun",
        )
        .on_hover_ui(analytics_details_ui);

        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("This helps us make the feature better.")
                        .small()
                        .weak(),
                );
                analytics_details_button(ui);
            });
        });
    });
}

/// "Details": the particulars of what is shared, on hover and on click.
///
/// Clickable as well as hoverable, like [`re_ui::UiExt::help_button`]: hover alone cannot be
/// reached without a pointer, and it gives the reader no way to keep the text open.
fn analytics_details_button(ui: &mut egui::Ui) -> egui::Response {
    let label = RichText::new("Details")
        .small()
        .underline()
        .color(ui.visuals().weak_text_color());
    let response = ui.link(label);
    let menu = egui::Popup::menu(&response).show(analytics_details_ui);

    if menu.is_some() {
        // Don't stack a tooltip on top of the open menu.
        response
    } else {
        response.on_hover_ui(analytics_details_ui)
    }
}

fn analytics_details_ui(ui: &mut egui::Ui) {
    ui.set_max_width(ANALYTICS_DETAILS_WIDTH);
    ui.label(format!("{ANALYTICS_DETAILS}{ANALYTICS_OPT_OUT}"));
}
