use super::Legend;

impl Default for Legend {
    fn default() -> Self {
        Self(crate::blueprint::datatypes::Legend {
            visible: true,
            location: None,
        })
    }
}

#[cfg(feature = "egui_plot")]
fn egui_to_u8(corner: egui_plot::Corner) -> u8 {
    match corner {
        egui_plot::Corner::LeftTop => 1,
        egui_plot::Corner::RightTop => 2,
        egui_plot::Corner::LeftBottom => 3,
        egui_plot::Corner::RightBottom => 4,
    }
}

#[cfg(feature = "egui_plot")]
fn u8_to_egui(corner: u8) -> Option<egui_plot::Corner> {
    match corner {
        1 => Some(egui_plot::Corner::LeftTop),
        2 => Some(egui_plot::Corner::RightTop),
        3 => Some(egui_plot::Corner::LeftBottom),
        4 => Some(egui_plot::Corner::RightBottom),
        _ => {
            re_log::warn_once!("Unknown legend corner value: {}", corner);
            None
        }
    }
}

#[cfg(feature = "egui_plot")]
impl Legend {
    pub fn corner(&self) -> Option<egui_plot::Corner> {
        self.0.location.and_then(u8_to_egui)
    }

    pub fn set_corner(&mut self, corner: egui_plot::Corner) {
        self.0.location = Some(egui_to_u8(corner));
    }

    pub fn to_str(corner: egui_plot::Corner) -> &'static str {
        match corner {
            egui_plot::Corner::LeftTop => "Left Top",
            egui_plot::Corner::RightTop => "Right Top",
            egui_plot::Corner::LeftBottom => "Left Bottom",
            egui_plot::Corner::RightBottom => "Right Bottom",
        }
    }
}
