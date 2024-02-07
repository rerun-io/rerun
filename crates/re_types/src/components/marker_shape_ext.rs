use super::MarkerShape;

impl MarkerShape {
    pub const CIRCLE: u8 = 1;
    pub const DIAMOND: u8 = 2;
    pub const SQUARE: u8 = 3;
    pub const CROSS: u8 = 4;
    pub const PLUS: u8 = 5;
    pub const UP: u8 = 6;
    pub const DOWN: u8 = 7;
    pub const LEFT: u8 = 8;
    pub const RIGHT: u8 = 9;
    pub const ASTERISK: u8 = 10;
}

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
fn u8_to_egui(marker: u8) -> egui_plot::MarkerShape {
    match marker {
        1 => egui_plot::MarkerShape::Circle,
        2 => egui_plot::MarkerShape::Diamond,
        3 => egui_plot::MarkerShape::Square,
        4 => egui_plot::MarkerShape::Cross,
        5 => egui_plot::MarkerShape::Plus,
        6 => egui_plot::MarkerShape::Up,
        7 => egui_plot::MarkerShape::Down,
        8 => egui_plot::MarkerShape::Left,
        9 => egui_plot::MarkerShape::Right,
        10 => egui_plot::MarkerShape::Asterisk,
        _ => {
            re_log::error_once!("Could not interpret {marker} as egui_plot::MarkerShape.");
            // Fall back on Circle
            egui_plot::MarkerShape::Circle
        }
    }
}

#[cfg(feature = "egui_plot")]
impl From<egui_plot::MarkerShape> for MarkerShape {
    #[inline]
    fn from(shape: egui_plot::MarkerShape) -> Self {
        Self(egui_to_u8(shape))
    }
}

#[cfg(feature = "egui_plot")]
impl From<MarkerShape> for egui_plot::MarkerShape {
    #[inline]
    fn from(value: MarkerShape) -> Self {
        u8_to_egui(value.0)
    }
}

impl Default for MarkerShape {
    #[inline]
    fn default() -> Self {
        Self(1)
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

    pub fn all_markers() -> Vec<MarkerShape> {
        (1..=10).map(MarkerShape).collect()
    }
}
