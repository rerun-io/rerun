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

#[cfg(target_os = "macos")]
pub const APP_ICON: Icon = Icon::new(
    "app_icon_mac",
    include_bytes!("../data/icons/app_icon_mac.png"),
);
#[cfg(target_os = "windows")]
pub const APP_ICON: Icon = Icon::new(
    "app_icon_windows",
    include_bytes!("../data/icons/app_icon_windows.png"),
);

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

pub const VISIBLE: Icon = Icon::new("visible", include_bytes!("../data/icons/visible.png"));
pub const INVISIBLE: Icon = Icon::new("invisible", include_bytes!("../data/icons/invisible.png"));

pub const ADD: Icon = Icon::new("add", include_bytes!("../data/icons/add.png"));
pub const REMOVE: Icon = Icon::new("remove", include_bytes!("../data/icons/remove.png"));

pub const RESET: Icon = Icon::new("reset", include_bytes!("../data/icons/reset.png"));

pub const CLOSE: Icon = Icon::new("close", include_bytes!("../data/icons/close.png"));

pub const GEAR: Icon = Icon::new("gear", include_bytes!("../data/icons/gear.png"));

pub const SPACE_VIEW_TEXT: Icon = Icon::new(
    "spaceview_text",
    include_bytes!("../data/icons/spaceview_text.png"),
);
pub const SPACE_VIEW_3D: Icon = Icon::new(
    "spaceview_3d",
    include_bytes!("../data/icons/spaceview_3d.png"),
);
pub const SPACE_VIEW_CHART: Icon = Icon::new(
    "spaceview_chart",
    include_bytes!("../data/icons/spaceview_chart.png"),
);
pub const SPACE_VIEW_SCATTERPLOT: Icon = Icon::new(
    "spaceview_scatterplot",
    include_bytes!("../data/icons/spaceview_scatterplot.png"),
);
pub const SPACE_VIEW_RAW: Icon = Icon::new(
    "spaceview_raw",
    include_bytes!("../data/icons/spaceview_raw.png"),
);
pub const SPACE_VIEW_TENSOR: Icon = Icon::new(
    "spaceview_tensor",
    include_bytes!("../data/icons/spaceview_tensor.png"),
);
pub const SPACE_VIEW_HISTOGRAM: Icon = Icon::new(
    "spaceview_histogram",
    include_bytes!("../data/icons/spaceview_histogram.png"),
);

pub const CONTAINER: Icon = Icon::new("container", include_bytes!("../data/icons/container.png"));
