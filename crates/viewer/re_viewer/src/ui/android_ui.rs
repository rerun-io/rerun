//! Android-specific UI adaptations for the Rerun Viewer.
//!
//! This module provides touch-friendly style adjustments and a connection info
//! banner that shows the gRPC server URL when running on Android.

/// egui temp-data key for the gRPC connection URL.
const GRPC_URL_KEY: &str = "android_grpc_url";

/// Apply Android-specific style adjustments to the egui context.
///
/// Call this once during app initialization to configure egui for touch-friendly
/// interaction on Android devices (larger hit targets, wider scrollbars, etc.).
pub fn apply_android_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Increase interaction target sizes for touch
    style.interaction.interact_radius = 12.0; // Default is 5.0
    style.interaction.resize_grab_radius_side = 12.0;
    style.interaction.resize_grab_radius_corner = 16.0;

    // Larger spacing for touch targets
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.item_spacing = egui::vec2(10.0, 6.0);

    // Larger scroll bar for touch
    style.spacing.scroll.bar_width = 12.0;
    style.spacing.scroll.handle_min_length = 32.0;

    ctx.set_style(style);
}

/// Show a small info banner at the bottom of the screen with the gRPC server URL.
///
/// This lets the user know which address to connect to from their SDK.
/// The URL is stored in egui's temp data by the Android entry point.
pub fn grpc_connection_banner(ui: &mut egui::Ui) {
    let connect_url: Option<String> = ui
        .ctx()
        .data(|d| d.get_temp::<Option<String>>(egui::Id::new(GRPC_URL_KEY)))
        .flatten();

    let banner_text = if let Some(url) = &connect_url {
        format!("gRPC server ready  \u{2022}  {url}")
    } else {
        format!(
            "gRPC server ready on port {}  \u{2022}  could not determine device IP",
            re_grpc_server::DEFAULT_SERVER_PORT,
        )
    };

    egui::TopBottomPanel::bottom("android_grpc_banner")
        .resizable(false)
        .frame(egui::Frame {
            fill: egui::Color32::from_rgb(30, 30, 30),
            inner_margin: egui::Margin::symmetric(12, 8),
            ..Default::default()
        })
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                // Green dot to indicate server is running
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(rect.center(), 4.0, egui::Color32::from_rgb(76, 175, 80));

                let response = ui.label(
                    egui::RichText::new(&banner_text)
                        .color(egui::Color32::from_rgb(200, 200, 200))
                        .size(12.0),
                );

                if let Some(url) = &connect_url {
                    response.on_hover_text(format!(
                        "Connect from Python:\n  import rerun as rr\n  rr.init(\"my_app\")\n  rr.connect_grpc(\"{url}\")"
                    ));
                }
            });
        });
}
