//! The [`LinkButton`] "smart link" button widget.

use egui::IntoAtoms;

use crate::button::ReButton;
use crate::{UiExt as _, icons};

/// A component wrapping [`ReButton`], optimized for showing links (has a built-in copy button).
pub struct LinkButton {
    url: String,
    atoms: egui::Atoms<'static>,
    wrap_mode: egui::TextWrapMode,
    tint_icons: bool,
}

impl LinkButton {
    /// Pass in a valid url and atoms representing the contents of the link.
    pub fn new(url: impl Into<String>, atoms: impl IntoAtoms<'static>) -> Self {
        Self {
            url: url.into(),
            atoms: atoms.into_atoms(),
            wrap_mode: egui::TextWrapMode::Extend,
            tint_icons: true,
        }
    }

    /// Override wrap mode.
    #[inline]
    pub fn wrap_mode(mut self, wrap_mode: egui::TextWrapMode) -> Self {
        self.wrap_mode = wrap_mode;
        self
    }

    /// Should the icons be tinted to the text color?
    #[inline]
    pub fn tint_icons(mut self, tint: bool) -> Self {
        self.tint_icons = tint;
        self
    }

    /// The button's content atoms, for consumers that render the content themselves.
    pub fn into_atoms(self) -> egui::Atoms<'static> {
        self.atoms
    }

    /// Show the button.
    ///
    /// Clicks are handled via [`egui::Context::open_url`].
    pub fn show_atom(self, ui: &mut egui::Ui) -> egui::AtomLayoutResponse {
        let Self {
            url,
            mut atoms,
            wrap_mode,
            tint_icons,
        } = self;

        let icon_size = ui.tokens().small_icon_size;

        atoms.map_images(|image| image.fit_to_exact_size(icon_size));

        let (mut response, copy_response) =
            ReButton::with_hover_icon_button(ui, ReButton::icon(icons::COPY).ghost(), || {
                ReButton::from_button(egui::Button::new(atoms.clone()).wrap_mode(wrap_mode))
                    .ghost()
                    .tiny()
                    .image_tint_follows_text_color(tint_icons)
            });
        let copy_response = copy_response.map(|r| r.on_hover_text("Copy link"));

        response.response = response
            .response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(url.clone());

        if copy_response.as_ref().is_some_and(|r| r.clicked()) {
            ui.copy_text(url.clone());
            re_log::info!("Link copied!");
        }

        if response.clicked() {
            let new_tab = response.clicked_with_open_in_background();
            ui.open_url(egui::OpenUrl { url, new_tab });
        }

        response
    }
}

impl egui::Widget for LinkButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show_atom(ui).response
    }
}
