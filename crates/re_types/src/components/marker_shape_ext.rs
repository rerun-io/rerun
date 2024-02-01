use super::MarkerShape;

#[cfg(feature = "egui_plot")]
fn egui_to_u8(marker: egui_plot::MarkerShape) -> u8 {
    match marker {
        egui_plot::MarkerShape::Circle => 1,
        egui_plot::MarkerShape::Diamond => 2,
        egui_plot::MarkerShape::Square => 3,
        egui_plot::MarkerShape::Cross => 4,
        egui_plot::MarkerShape::Plus => 5,
        egui_plot::MarkerShape::Up => 6,
        egui_plot::MarkerShape::Down => 7,
        egui_plot::MarkerShape::Left => 8,
        egui_plot::MarkerShape::Right => 9,
        egui_plot::MarkerShape::Asterisk => 10,
    }
}

#[cfg(feature = "egui_plot")]
fn u8_to_egui(marker: u8) -> Result<egui_plot::MarkerShape, String> {
    match marker {
        1 => Ok(egui_plot::MarkerShape::Circle),
        2 => Ok(egui_plot::MarkerShape::Diamond),
        3 => Ok(egui_plot::MarkerShape::Square),
        4 => Ok(egui_plot::MarkerShape::Cross),
        5 => Ok(egui_plot::MarkerShape::Plus),
        6 => Ok(egui_plot::MarkerShape::Up),
        7 => Ok(egui_plot::MarkerShape::Down),
        8 => Ok(egui_plot::MarkerShape::Left),
        9 => Ok(egui_plot::MarkerShape::Right),
        10 => Ok(egui_plot::MarkerShape::Asterisk),
        _ => Err("Could not interpret {marker} as egui_plot::MarkerShape.".to_owned()),
    }
}

#[cfg(feature = "egui_plot")]
impl From<egui_plot::MarkerShape> for MarkerShape {
    #[inline]
    fn from(shape: egui_plot::MarkerShape) -> Self {
        Self(egui_to_u8(shape))
    }
}

impl TryFrom<MarkerShape> for egui_plot::MarkerShape {
    type Error = String;

    #[inline]
    fn try_from(value: MarkerShape) -> Result<Self, Self::Error> {
        u8_to_egui(value.0)
    }
}

impl MarkerShape {
    pub fn as_str(&self) -> &'static str {
        match self.0 {
            1 => "Circle",
            2 => "Diamond",
            3 => "Square",
            4 => "Cross",
            5 => "Plus",
            6 => "Up",
            7 => "Down",
            8 => "Left",
            9 => "Right",
            10 => "Asterisk",
            _ => "Unknown",
        }
    }
}
