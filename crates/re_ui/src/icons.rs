use egui::{Image, ImageSource};

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

    pub fn as_image(&self) -> Image<'static> {
        Image::new(ImageSource::Bytes {
            uri: self.id.into(),
            bytes: self.png_bytes.into(),
        })
    }
}

pub const RERUN_MENU: Icon =
    Icon::new("rerun_menu", include_bytes!("../data/icons/rerun_menu.png"));

pub const RERUN_IO_TEXT: Icon = Icon::new("rerun_io", include_bytes!("../data/icons/rerun_io.png"));

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

pub const ADD_BIG: Icon = Icon::new("add_big", include_bytes!("../data/icons/add_big.png"));
pub const REMOVE: Icon = Icon::new("remove", include_bytes!("../data/icons/remove.png"));

pub const RESET: Icon = Icon::new("reset", include_bytes!("../data/icons/reset.png"));

pub const CLOSE: Icon = Icon::new("close", include_bytes!("../data/icons/close.png"));

/// Used for HTTP URLs that leads out of the app.
///
/// Remember to also use `.on_hover_cursor(egui::CursorIcon::PointingHand)`
/// and `.on_hover_text(url)`.
pub const EXTERNAL_LINK: Icon = Icon::new(
    "external_link",
    include_bytes!("../data/icons/external_link.png"),
);
pub const DISCORD: Icon = Icon::new("discord", include_bytes!("../data/icons/discord.png"));

pub const CONTAINER_HORIZONTAL: Icon = Icon::new(
    "container_horizontal",
    include_bytes!("../data/icons/container_horizontal.png"),
);
pub const CONTAINER_GRID: Icon = Icon::new(
    "container_grid",
    include_bytes!("../data/icons/container_grid.png"),
);
pub const CONTAINER_TABS: Icon = Icon::new(
    "container_tabs",
    include_bytes!("../data/icons/container_tabs.png"),
);
pub const CONTAINER_VERTICAL: Icon = Icon::new(
    "container_vertical",
    include_bytes!("../data/icons/container_vertical.png"),
);

pub const SPACE_VIEW_2D: Icon = Icon::new(
    "spaceview_2d",
    include_bytes!("../data/icons/spaceview_2d.png"),
);
pub const SPACE_VIEW_3D: Icon = Icon::new(
    "spaceview_3d",
    include_bytes!("../data/icons/spaceview_3d.png"),
);
pub const SPACE_VIEW_DATAFRAME: Icon = Icon::new(
    "spaceview_dataframe",
    include_bytes!("../data/icons/spaceview_dataframe.png"),
);
pub const SPACE_VIEW_GENERIC: Icon = Icon::new(
    "spaceview_unknown",
    include_bytes!("../data/icons/spaceview_generic.png"),
);
pub const SPACE_VIEW_HISTOGRAM: Icon = Icon::new(
    "spaceview_histogram",
    include_bytes!("../data/icons/spaceview_histogram.png"),
);
pub const SPACE_VIEW_LOG: Icon = Icon::new(
    "spaceview_text",
    include_bytes!("../data/icons/spaceview_log.png"),
);
pub const SPACE_VIEW_TENSOR: Icon = Icon::new(
    "spaceview_tensor",
    include_bytes!("../data/icons/spaceview_tensor.png"),
);
pub const SPACE_VIEW_TEXT: Icon = Icon::new(
    "spaceview_text",
    include_bytes!("../data/icons/spaceview_text.png"),
);
pub const SPACE_VIEW_TIMESERIES: Icon = Icon::new(
    "spaceview_chart",
    include_bytes!("../data/icons/spaceview_timeseries.png"),
);
pub const SPACE_VIEW_UNKNOWN: Icon = Icon::new(
    "spaceview_unknown",
    include_bytes!("../data/icons/spaceview_unknown.png"),
);

pub const CONTAINER: Icon = Icon::new("container", include_bytes!("../data/icons/container.png"));

pub const STORE: Icon = Icon::new("store", include_bytes!("../data/icons/store.png"));

pub const WELCOME_SCREEN_CONFIGURE: Icon = Icon::new(
    "welcome_screen_configure",
    include_bytes!("../data/images/onboarding-configure.png"),
);

pub const WELCOME_SCREEN_LIVE_DATA: Icon = Icon::new(
    "welcome_screen_live_data",
    include_bytes!("../data/images/onboarding-live-data.png"),
);

pub const WELCOME_SCREEN_RECORDED_DATA: Icon = Icon::new(
    "welcome_screen_recorded_data",
    include_bytes!("../data/images/onboarding-recorded-data.png"),
);

pub const WELCOME_SCREEN_EXAMPLES: Icon = Icon::new(
    "welcome_screen_examples",
    include_bytes!("../data/images/onboarding-examples.jpg"),
);
