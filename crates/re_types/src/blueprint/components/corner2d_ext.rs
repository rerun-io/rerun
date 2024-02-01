use super::Corner2D;

impl Default for Corner2D {
    fn default() -> Self {
        // Default to right bottom for the general case.
        // (Each space view / item using this may still decide on its own default though)
        Self(4)
    }
}

#[cfg(feature = "egui_plot")]
impl TryFrom<Corner2D> for egui_plot::Corner {
    type Error = ();

    fn try_from(corner: Corner2D) -> Result<Self, Self::Error> {
        match corner.0 {
            1 => Ok(egui_plot::Corner::LeftTop),
            2 => Ok(egui_plot::Corner::RightTop),
            3 => Ok(egui_plot::Corner::LeftBottom),
            4 => Ok(egui_plot::Corner::RightBottom),
            _ => {
                re_log::warn_once!("Unknown corner value: {}", corner);
                Err(())
            }
        }
    }
}

#[cfg(feature = "egui_plot")]
impl From<egui_plot::Corner> for Corner2D {
    fn from(corner: egui_plot::Corner) -> Self {
        Self(match corner {
            egui_plot::Corner::LeftTop => 1,
            egui_plot::Corner::RightTop => 2,
            egui_plot::Corner::LeftBottom => 3,
            egui_plot::Corner::RightBottom => 4,
        })
    }
}

#[cfg(feature = "egui_plot")]
impl std::fmt::Display for Corner2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match egui_plot::Corner::try_from(*self) {
            Ok(egui_plot::Corner::LeftTop) => write!(f, "Left Top"),
            Ok(egui_plot::Corner::RightTop) => write!(f, "Right Top"),
            Ok(egui_plot::Corner::LeftBottom) => write!(f, "Left Bottom"),
            Ok(egui_plot::Corner::RightBottom) => write!(f, "Right Bottom"),
            Err(_) => write!(f, "Unknown corner value: {}", self.0),
        }
    }
}
