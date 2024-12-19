use egui::{Image, ImageSource};

#[derive(Clone, Copy, Debug)]
pub struct Icon {
    /// Human readable unique id
    pub id: &'static str,

    pub png_bytes: &'static [u8],
}

impl Icon {
    #[inline]
    pub const fn new(id: &'static str, png_bytes: &'static [u8]) -> Self {
        Self { id, png_bytes }
    }

    #[inline]
    pub fn as_image_source(&self) -> ImageSource<'static> {
        ImageSource::Bytes {
            uri: self.id.into(),
            bytes: self.png_bytes.into(),
        }
    }

    #[inline]
    pub fn as_image(&self) -> Image<'static> {
        // Default size is the same size as the source data specifies
        const ICON_SCALE: f32 = 0.5; // Because we save all icons as 2x
        Image::new(self.as_image_source()).fit_to_original_size(ICON_SCALE)
    }
}

impl From<&'static Icon> for Image<'static> {
    #[inline]
    fn from(icon: &'static Icon) -> Self {
        icon.as_image()
    }
}

/// Macro to create an [`Icon`], using the file path as the id.
///
/// This avoids specifying the id manually, which is error-prone (duplicate IDs lead to silent
/// display bugs).
macro_rules! icon_from_path {
    ($path:literal) => {
        Icon::new($path, include_bytes!($path))
    };
}

pub const RERUN_MENU: Icon = icon_from_path!("../data/icons/rerun_menu.png");

pub const RERUN_IO_TEXT: Icon = icon_from_path!("../data/icons/rerun_io.png");

pub const PLAY: Icon = icon_from_path!("../data/icons/play.png");
pub const FOLLOW: Icon = icon_from_path!("../data/icons/follow.png");
pub const PAUSE: Icon = icon_from_path!("../data/icons/pause.png");
pub const ARROW_LEFT: Icon = icon_from_path!("../data/icons/arrow_left.png");
pub const ARROW_RIGHT: Icon = icon_from_path!("../data/icons/arrow_right.png");
pub const ARROW_DOWN: Icon = icon_from_path!("../data/icons/arrow_down.png");
pub const LOOP: Icon = icon_from_path!("../data/icons/loop.png");

pub const NOTIFICATION: Icon = icon_from_path!("../data/icons/notification.png");
pub const RIGHT_PANEL_TOGGLE: Icon = icon_from_path!("../data/icons/right_panel_toggle.png");
pub const BOTTOM_PANEL_TOGGLE: Icon = icon_from_path!("../data/icons/bottom_panel_toggle.png");
pub const LEFT_PANEL_TOGGLE: Icon = icon_from_path!("../data/icons/left_panel_toggle.png");

pub const MINIMIZE: Icon = icon_from_path!("../data/icons/minimize.png");
pub const MAXIMIZE: Icon = icon_from_path!("../data/icons/maximize.png");

pub const COLLAPSE: Icon = icon_from_path!("../data/icons/collapse.png");
pub const EXPAND: Icon = icon_from_path!("../data/icons/expand.png");
pub const COLUMN_VISIBILITY: Icon = icon_from_path!("../data/icons/column_visibility.png");

pub const VISIBLE: Icon = icon_from_path!("../data/icons/visible.png");
pub const INVISIBLE: Icon = icon_from_path!("../data/icons/invisible.png");

pub const ADD: Icon = icon_from_path!("../data/icons/add.png");

pub const REMOVE: Icon = icon_from_path!("../data/icons/remove.png");

pub const RESET: Icon = icon_from_path!("../data/icons/reset.png");

pub const EDIT: Icon = icon_from_path!("../data/icons/edit.png");
pub const MORE: Icon = icon_from_path!("../data/icons/more.png");

pub const CLOSE: Icon = icon_from_path!("../data/icons/close.png");

/// Used for HTTP URLs that lead out of the app.
///
/// Remember to also use `.on_hover_cursor(egui::CursorIcon::PointingHand)`,
/// but don't add `.on_hover_text(url)`.
pub const EXTERNAL_LINK: Icon = icon_from_path!("../data/icons/external_link.png");
pub const DISCORD: Icon = icon_from_path!("../data/icons/discord.png");

pub const CONTAINER_HORIZONTAL: Icon = icon_from_path!("../data/icons/container_horizontal.png");
pub const CONTAINER_GRID: Icon = icon_from_path!("../data/icons/container_grid.png");
pub const CONTAINER_TABS: Icon = icon_from_path!("../data/icons/container_tabs.png");
pub const CONTAINER_VERTICAL: Icon = icon_from_path!("../data/icons/container_vertical.png");

pub const VIEW_2D: Icon = icon_from_path!("../data/icons/view_2d.png");
pub const VIEW_3D: Icon = icon_from_path!("../data/icons/view_3d.png");
pub const VIEW_DATAFRAME: Icon = icon_from_path!("../data/icons/view_dataframe.png");
pub const VIEW_GRAPH: Icon = icon_from_path!("../data/icons/view_graph.png");
pub const VIEW_GENERIC: Icon = icon_from_path!("../data/icons/view_generic.png");
pub const VIEW_HISTOGRAM: Icon = icon_from_path!("../data/icons/view_histogram.png");
pub const VIEW_LOG: Icon = icon_from_path!("../data/icons/view_log.png");
pub const VIEW_MAP: Icon = icon_from_path!("../data/icons/view_map.png");
pub const VIEW_TENSOR: Icon = icon_from_path!("../data/icons/view_tensor.png");
pub const VIEW_TEXT: Icon = icon_from_path!("../data/icons/view_text.png");
pub const VIEW_TIMESERIES: Icon = icon_from_path!("../data/icons/view_timeseries.png");
pub const VIEW_UNKNOWN: Icon = icon_from_path!("../data/icons/view_unknown.png");

pub const GROUP: Icon = icon_from_path!("../data/icons/group.png");
pub const ENTITY: Icon = icon_from_path!("../data/icons/entity.png");
pub const ENTITY_EMPTY: Icon = icon_from_path!("../data/icons/entity_empty.png");

/// Link within the viewer
pub const INTERNAL_LINK: Icon = icon_from_path!("../data/icons/link.png");

pub const COMPONENT_TEMPORAL: Icon = icon_from_path!("../data/icons/component.png");
pub const COMPONENT_STATIC: Icon = icon_from_path!("../data/icons/component_static.png");

pub const APPLICATION: Icon = icon_from_path!("../data/icons/application.png");
pub const DATA_SOURCE: Icon = icon_from_path!("../data/icons/data_source.png");
pub const RECORDING: Icon = icon_from_path!("../data/icons/recording.png");
pub const BLUEPRINT: Icon = icon_from_path!("../data/icons/blueprint.png");

pub const GITHUB: Icon = icon_from_path!("../data/icons/github.png");

pub const VIDEO_ERROR: Icon = icon_from_path!("../data/icons/video_error.png");

// drag and drop icons
pub const DND_ADD_NEW: Icon = icon_from_path!("../data/icons/dnd_add_new.png");
pub const DND_ADD_TO_EXISTING: Icon = icon_from_path!("../data/icons/dnd_add_to_existing.png");
pub const DND_MOVE: Icon = icon_from_path!("../data/icons/dnd_move.png");

/// `>`
pub const BREADCRUMBS_SEPARATOR: Icon = icon_from_path!("../data/icons/breadcrumbs_separator.png");
