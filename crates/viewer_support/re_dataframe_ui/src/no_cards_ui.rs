//! The screen the cards view shows for a table without a card layout.

use re_agent_tools::ModelCategory;
use re_ui::{ButtonExt as _, UiExt as _, icons};
use re_viewer_context::{AppContext, SystemCommand, SystemCommandSender as _};

/// Where the Python API for card layouts is documented.
const CARD_LAYOUT_DOCS_URL: &str = "https://rerun.io/docs/viewer/browse-datasets/table-layout";

/// What the button that sets up the cards asks the agent.
const SET_UP_CARDS_PROMPT: &str = "Create a blueprint for this dataset";

/// The widest the explanation gets before it wraps.
const EXPLANATION_WIDTH: f32 = 420.0;

/// The space above the sketch, which puts the content in the upper part of the area.
const TOP_SPACE: f32 = 32.0;

/// Explains what cards are, and offers to set them up with the agent or in Python.
pub fn no_cards_ui(ui: &mut egui::Ui, ctx: &AppContext<'_>) {
    ui.vertical_centered(|ui| {
        ui.add_space(TOP_SPACE);

        card_sketch_ui(ui);

        // The sketch, the heading and the explanation are one message, so the gaps between them
        // are small. The one large gap is before the actions.
        ui.add_space(16.0);
        ui.label(egui::RichText::new("No cards yet").heading().strong());
        ui.add_space(4.0);

        ui.allocate_ui_with_layout(
            egui::vec2(EXPLANATION_WIDTH.min(ui.available_width()), 0.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            "Cards show each row with a preview and fields from your recordings. \
                             The agent can read your data and set them up for you.",
                        )
                        .weak(),
                    )
                    .halign(egui::Align::Center)
                    .wrap(),
                );
            },
        );

        ui.add_space(24.0);

        if ui
            .add(
                egui::Button::new((icons::AGENT, "Set up cards with agent"))
                    .blue()
                    .size_small(),
            )
            .on_hover_text("Ask the agent in the agent panel to set up the cards")
            .clicked()
        {
            ctx.command_sender()
                .send_system(SystemCommand::TriggerPrompt {
                    model: ModelCategory::Fast.into(),
                    prompt: SET_UP_CARDS_PROMPT.to_owned(),
                });
        }

        ui.add_space(12.0);

        ui.re_hyperlink("Write the layout in Python", CARD_LAYOUT_DOCS_URL, true);
    });
}

/// Paints three placeholder cards, so the reader sees what a card is before any exists.
///
/// The middle card is the clearest and the outer two fade, so the row reads as one picture and
/// not as three things to click.
fn card_sketch_ui(ui: &mut egui::Ui) {
    const CARD_SIZE: egui::Vec2 = egui::vec2(124.0, 106.0);
    const CARD_GAP: f32 = 12.0;
    const CARD_PADDING: f32 = 10.0;
    const PREVIEW_HEIGHT: f32 = 54.0;
    const LINE_HEIGHT: f32 = 5.0;

    /// The height of the play mark in the middle card.
    const PLAY_SIZE: f32 = 12.0;

    /// How much of each card shows, from left to right.
    const OPACITIES: [f32; 3] = [0.45, 1.0, 0.45];

    /// How much of the card's inner width its title and its field take.
    const LINE_WIDTHS: [f32; 2] = [0.5, 0.8];

    let tokens = ui.tokens();

    let size = egui::vec2(
        OPACITIES.len() as f32 * CARD_SIZE.x + (OPACITIES.len() - 1) as f32 * CARD_GAP,
        CARD_SIZE.y,
    );
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    // Dark ink on a light page reads much stronger than light ink on a dark page, so the light
    // theme draws the sketch with less of it.
    let theme_strength = match ui.ctx().theme() {
        egui::Theme::Dark => 1.0,
        egui::Theme::Light => 0.4,
    };

    let painter = ui.painter();

    for (index, opacity) in OPACITIES.into_iter().enumerate() {
        let card = egui::Rect::from_min_size(
            rect.min + egui::vec2(index as f32 * (CARD_SIZE.x + CARD_GAP), 0.0),
            CARD_SIZE,
        );
        let opacity = opacity * theme_strength;
        let ink = |strength: f32| tokens.text_subdued.gamma_multiply(strength * opacity);

        painter.rect(
            card,
            8.0,
            tokens.faint_bg_color.gamma_multiply(opacity),
            egui::Stroke::new(1.0, ink(0.35)),
            egui::StrokeKind::Inside,
        );

        let inner = card.shrink(CARD_PADDING);

        let preview =
            egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), PREVIEW_HEIGHT));
        painter.rect_filled(preview, 4.0, ink(0.18));

        // Only the middle card plays, which marks the block as a preview of a recording.
        if index == OPACITIES.len() / 2 {
            let center = preview.center();
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + egui::vec2(-PLAY_SIZE * 0.4, -PLAY_SIZE * 0.5),
                    center + egui::vec2(PLAY_SIZE * 0.5, 0.0),
                    center + egui::vec2(-PLAY_SIZE * 0.4, PLAY_SIZE * 0.5),
                ],
                ink(0.45),
                egui::Stroke::NONE,
            ));
        }

        let mut line_top = preview.bottom() + CARD_PADDING;
        for (line_width, strength) in std::iter::zip(LINE_WIDTHS, [0.55, 0.25]) {
            let line = egui::Rect::from_min_size(
                egui::pos2(inner.left(), line_top),
                egui::vec2(inner.width() * line_width, LINE_HEIGHT),
            );
            painter.rect_filled(line, LINE_HEIGHT / 2.0, ink(strength));
            line_top += LINE_HEIGHT + 6.0;
        }
    }
}
