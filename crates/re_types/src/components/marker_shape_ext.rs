use super::MarkerShape;

// TODO(#3384): This should be generated
#[allow(non_upper_case_globals)]
impl MarkerShape {
    pub const Circle: u8 = 1;
    pub const Diamond: u8 = 2;
    pub const Square: u8 = 3;
    pub const Cross: u8 = 4;
    pub const Plus: u8 = 5;
    pub const Up: u8 = 6;
    pub const Down: u8 = 7;
    pub const Left: u8 = 8;
    pub const Right: u8 = 9;
    pub const Asterisk: u8 = 10;
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
