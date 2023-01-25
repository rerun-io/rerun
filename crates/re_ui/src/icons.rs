#[derive(Clone, Copy, Debug)]
pub struct Icon {
    /// Human readable unique id
    pub id: &'static str,

    pub png_bytes: &'static [u8],
}

impl Icon {
    pub const fn new(id: &'static str, png_bytes: &'static [u8]) -> Self {
        Self { id, png_bytes }
    }
}

pub const RERUN_MENU: Icon =
    Icon::new("rerun_menu", include_bytes!("../data/icons/rerun_menu.png"));

pub const PLAY: Icon = Icon::new("play", include_bytes!("../data/icons/play.png"));
pub const FOLLOW: Icon = Icon::new("follow", include_bytes!("../data/icons/follow.png"));
pub const PAUSE: Icon = Icon::new("pause", include_bytes!("../data/icons/pause.png"));
pub const ARROW_LEFT: Icon =
    Icon::new("arrow_left", include_bytes!("../data/icons/arrow_left.png"));
pub const ARROW_RIGHT: Icon = Icon::new(
    "arrow_right",
    include_bytes!("../data/icons/arrow_right.png"),
);
pub const LOOP: Icon = Icon::new("loop", include_bytes!("../data/icons/loop.png"));

pub const RIGHT_PANEL_TOGGLE: Icon = Icon::new(
    "right_panel_toggle",
    include_bytes!("../data/icons/right_panel_toggle.png"),
);
pub const BOTTOM_PANEL_TOGGLE: Icon = Icon::new(
    "bottom_panel_toggle",
    include_bytes!("../data/icons/bottom_panel_toggle.png"),
);
pub const LEFT_PANEL_TOGGLE: Icon = Icon::new(
    "left_panel_toggle",
    include_bytes!("../data/icons/left_panel_toggle.png"),
);

pub const MINIMIZE: Icon = Icon::new("minimize", include_bytes!("../data/icons/minimize.png"));
pub const MAXIMIZE: Icon = Icon::new("maximize", include_bytes!("../data/icons/maximize.png"));

pub const ADD: Icon = Icon::new("add", include_bytes!("../data/icons/add.png"));
pub const RESET: Icon = Icon::new("reset", include_bytes!("../data/icons/reset.png"));
