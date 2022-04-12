use eframe::egui;
use log_types::{Data, LogId, ObjectPath, TimeValue};

use crate::log_db::LogDb;

/// Common things needed by many parts of the viewer.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ViewerContext {
    /// For displaying images effectively.
    #[serde(skip)]
    pub image_cache: crate::misc::ImageCache,

    /// The current time.
    pub time_control: crate::TimeControl,

    /// Currently selected thing, shown in the context menu.
    pub selection: Selection,
}

impl ViewerContext {
    /// Button to select the current space.
    pub fn space_button(
        &mut self,
        ui: &mut eframe::egui::Ui,
        space: &ObjectPath,
    ) -> egui::Response {
        // TODO: common hover-effect of all buttons for the same space!
        let response = ui.selectable_label(self.selection.is_space(space), space.to_string());
        if response.clicked() {
            self.selection = Selection::Space(space.clone());
        }
        response
    }

    pub fn time_button(
        &mut self,
        ui: &mut eframe::egui::Ui,
        time_source: &str,
        value: TimeValue,
    ) -> egui::Response {
        let is_selected =
            self.time_control.source() == time_source && self.time_control.time() == Some(value);

        let response = ui.selectable_label(is_selected, value.to_string());
        if response.clicked() {
            self.time_control
                .set_source_and_time(time_source.to_string(), value);
            self.time_control.pause();
        }
        response
    }

    #[allow(clippy::unused_self)]
    pub fn object_color(&self, log_db: &LogDb, path: &ObjectPath) -> egui::Color32 {
        if let Some(time) = self.time_control.time() {
            let color_path = path.sibling("color");
            if let Some(color_msg) = log_db.latest(self.time_control.source(), time, &color_path) {
                if let Data::Color([r, g, b, a]) = &color_msg.data {
                    return egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
                } else {
                    tracing::warn!(
                        "Expected color data in {:?}; found {:?}",
                        color_path,
                        color_msg.data
                    );
                }
            }
        }

        use rand::rngs::SmallRng;
        use rand::{Rng, SeedableRng};

        // TODO: ignore `TempId` id:s!
        let mut small_rng = SmallRng::seed_from_u64(egui::util::hash(path));

        // TODO: OKLab
        let hsva = egui::color::Hsva {
            h: small_rng.gen(),
            s: small_rng.gen_range(0.35..=0.55_f32).sqrt(),
            v: small_rng.gen_range(0.55..=0.80_f32).cbrt(),
            a: 1.0,
        };

        hsva.into()
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Selection {
    None,
    LogId(LogId),
    Space(ObjectPath),
}

impl Default for Selection {
    fn default() -> Self {
        Self::None
    }
}

impl Selection {
    pub fn is_space(&self, needle: &ObjectPath) -> bool {
        if let Self::Space(hay) = self {
            hay == needle
        } else {
            false
        }
    }
}
