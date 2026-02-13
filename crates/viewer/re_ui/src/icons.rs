use std::fmt::Formatter;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};

use egui::{Atom, Image, ImageSource};

use crate::DesignTokens;

#[derive(Clone, Copy)]
pub struct Icon {
    /// Human-readable unique id.
    ///
    /// This usually ends with `.png` or `.svg`.
    uri: &'static str,

    /// The raw contents of e.g. a PNG or SVG file.
    image_bytes: &'static [u8],
}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut hasher = DefaultHasher::new();
        self.image_bytes.hash(&mut hasher);
        let hash = hasher.finish();

        f.debug_struct("Icon")
            .field("uri", &self.uri)
            .field(
                "image_bytes",
                &format!("len: {}, hash: {:#x}", self.image_bytes.len(), hash),
            )
            .finish()
    }
}

impl Icon {
    #[inline]
    pub const fn new(uri: &'static str, image_bytes: &'static [u8]) -> Self {
        Self { uri, image_bytes }
    }

    pub fn uri(&self) -> &'static str {
        self.uri
    }

    #[inline]
    pub fn as_image_source(&self) -> ImageSource<'static> {
        ImageSource::Bytes {
            uri: self.uri.into(),
            bytes: self.image_bytes.into(),
        }
    }

    pub fn load_image(
        &self,
        egui_ctx: &egui::Context,
        size_hint: egui::SizeHint,
    ) -> egui::load::ImageLoadResult {
        egui_ctx.include_bytes(self.uri(), self.image_bytes);
        egui_ctx.try_load_image(self.uri(), size_hint)
    }

    #[inline]
    pub fn as_image(&self) -> Image<'static> {
        let scale = if self.uri.ends_with(".svg") {
            1.0
        } else {
            0.5 // Because we save all png icons as 2x
        };
        // Default size is the same size as the source data specifies
        Image::new(self.as_image_source()).fit_to_original_size(scale)
    }

    #[inline]
    pub fn as_button(&self) -> egui::Button<'_> {
        egui::Button::image(self.as_image()).image_tint_follows_text_color(true)
    }

    #[inline]
    pub fn as_button_with_label(
        &self,
        tokens: &DesignTokens,
        label: impl Into<egui::WidgetText>,
    ) -> egui::Button<'_> {
        egui::Button::image_and_text(self.as_image().tint(tokens.label_button_icon_color), label)
    }
}

impl From<&'static Icon> for Image<'static> {
    #[inline]
    fn from(icon: &'static Icon) -> Self {
        icon.as_image()
    }
}

impl From<&Icon> for Atom<'static> {
    #[inline]
    fn from(icon: &Icon) -> Self {
        Atom::from(icon.as_image())
    }
}

impl From<Icon> for Atom<'static> {
    #[inline]
    fn from(icon: Icon) -> Self {
        Atom::from(icon.as_image())
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

pub const RERUN_MENU: Icon = icon_from_path!("../data/icons/rerun_menu.svg");

pub const RERUN_IO_TEXT: Icon = icon_from_path!("../data/icons/rerun_io.svg");

pub const HELP: Icon = icon_from_path!("../data/icons/help.svg");

pub const PLAY: Icon = icon_from_path!("../data/icons/play.svg");
pub const FOLLOW: Icon = icon_from_path!("../data/icons/follow.svg");
pub const PAUSE: Icon = icon_from_path!("../data/icons/pause.svg");
pub const ARROW_LEFT: Icon = icon_from_path!("../data/icons/arrow_left.svg");
pub const ARROW_RIGHT: Icon = icon_from_path!("../data/icons/arrow_right.svg");
pub const ARROW_UP: Icon = icon_from_path!("../data/icons/arrow_up.svg");
pub const ARROW_DOWN: Icon = icon_from_path!("../data/icons/arrow_down.svg");
pub const LOOP: Icon = icon_from_path!("../data/icons/loop.svg");
pub const FILTER: Icon = icon_from_path!("../data/icons/filter.svg");

pub const NOTIFICATION: Icon = icon_from_path!("../data/icons/notification.svg");
pub const RIGHT_PANEL_TOGGLE: Icon = icon_from_path!("../data/icons/right_panel_toggle.svg");
pub const BOTTOM_PANEL_TOGGLE: Icon = icon_from_path!("../data/icons/bottom_panel_toggle.svg");
pub const LEFT_PANEL_TOGGLE: Icon = icon_from_path!("../data/icons/left_panel_toggle.svg");

pub const MINIMIZE: Icon = icon_from_path!("../data/icons/minimize.svg");
pub const MAXIMIZE: Icon = icon_from_path!("../data/icons/maximize.svg");
pub const EXPAND: Icon = icon_from_path!("../data/icons/expand.svg");
pub const COLUMN_VISIBILITY: Icon = icon_from_path!("../data/icons/column_visibility.svg");

pub const VISIBLE: Icon = icon_from_path!("../data/icons/visible.svg");
pub const INVISIBLE: Icon = icon_from_path!("../data/icons/invisible.svg");

pub const ADD: Icon = icon_from_path!("../data/icons/add.svg");

pub const REMOVE: Icon = icon_from_path!("../data/icons/remove.svg");
pub const TRASH: Icon = icon_from_path!("../data/icons/trash.svg");

pub const RESET: Icon = icon_from_path!("../data/icons/reset.svg");

pub const EDIT: Icon = icon_from_path!("../data/icons/edit.svg");
pub const MORE: Icon = icon_from_path!("../data/icons/more.svg");

pub const CLOSE: Icon = icon_from_path!("../data/icons/close.svg");
pub const CLOSE_SMALL: Icon = icon_from_path!("../data/icons/close_small.svg");

/// Used for HTTP URLs that lead out of the app.
///
/// Remember to also use `.on_hover_cursor(egui::CursorIcon::PointingHand)`,
/// but don't add `.on_hover_text(url)`.
pub const EXTERNAL_LINK: Icon = icon_from_path!("../data/icons/external_link.svg");
pub const DISCORD: Icon = icon_from_path!("../data/icons/discord.svg");

pub const URL: Icon = icon_from_path!("../data/icons/url.svg");

pub const CONTAINER_HORIZONTAL: Icon = icon_from_path!("../data/icons/container_horizontal.svg");
pub const CONTAINER_GRID: Icon = icon_from_path!("../data/icons/container_grid.svg");
pub const CONTAINER_TABS: Icon = icon_from_path!("../data/icons/container_tabs.svg");
pub const CONTAINER_VERTICAL: Icon = icon_from_path!("../data/icons/container_vertical.svg");

pub const VIEW_2D: Icon = icon_from_path!("../data/icons/view_2d.svg");
pub const VIEW_3D: Icon = icon_from_path!("../data/icons/view_3d.svg");
pub const VIEW_DATAFRAME: Icon = icon_from_path!("../data/icons/view_dataframe.svg");
pub const VIEW_GRAPH: Icon = icon_from_path!("../data/icons/view_graph.svg");
pub const VIEW_GENERIC: Icon = icon_from_path!("../data/icons/view_generic.svg");
pub const VIEW_HISTOGRAM: Icon = icon_from_path!("../data/icons/view_histogram.svg");
pub const VIEW_LOG: Icon = icon_from_path!("../data/icons/view_log.svg");
pub const VIEW_MAP: Icon = icon_from_path!("../data/icons/view_map.svg");
pub const VIEW_TENSOR: Icon = icon_from_path!("../data/icons/view_tensor.svg");
pub const VIEW_TEXT: Icon = icon_from_path!("../data/icons/view_text.svg");
pub const VIEW_TIMESERIES: Icon = icon_from_path!("../data/icons/view_timeseries.svg");
pub const VIEW_UNKNOWN: Icon = icon_from_path!("../data/icons/view_unknown.svg");

pub const GROUP: Icon = icon_from_path!("../data/icons/group.svg");
pub const ENTITY: Icon = icon_from_path!("../data/icons/entity.svg");
pub const ENTITY_EMPTY: Icon = icon_from_path!("../data/icons/entity_empty.svg");
pub const ENTITY_RESERVED: Icon = icon_from_path!("../data/icons/entity_reserved.svg");
pub const ENTITY_RESERVED_EMPTY: Icon = icon_from_path!("../data/icons/entity_reserved_empty.svg");

/// Link within the viewer
pub const INTERNAL_LINK: Icon = icon_from_path!("../data/icons/internal_link.svg");

pub const COMPONENT_TEMPORAL: Icon = icon_from_path!("../data/icons/component.svg");
pub const COMPONENT_STATIC: Icon = icon_from_path!("../data/icons/component_static.svg");

pub const APPLICATION: Icon = icon_from_path!("../data/icons/application.svg");
pub const DATA_SOURCE: Icon = icon_from_path!("../data/icons/data_source.svg");
pub const HOME: Icon = icon_from_path!("../data/icons/home.svg");
pub const TABLE: Icon = icon_from_path!("../data/icons/table.svg");
pub const DATASET: Icon = icon_from_path!("../data/icons/dataset.svg");
pub const RECORDING: Icon = icon_from_path!("../data/icons/recording.svg");
pub const OPEN_RECORDING: Icon = icon_from_path!("../data/icons/open_recording.svg");
pub const BLUEPRINT: Icon = icon_from_path!("../data/icons/blueprint.svg");

pub const GITHUB: Icon = icon_from_path!("../data/icons/github.svg");

// Notifications:
pub const INFO: Icon = icon_from_path!("../data/icons/info.svg");
pub const WARNING: Icon = icon_from_path!("../data/icons/warn.svg");
pub const ERROR: Icon = icon_from_path!("../data/icons/error.svg");
pub const SUCCESS: Icon = icon_from_path!("../data/icons/success.svg");
pub const VIDEO_ERROR: Icon = icon_from_path!("../data/icons/video_error.svg");

// Drag and drop:
pub const DND_ADD_NEW: Icon = icon_from_path!("../data/icons/dnd_add_new.svg");
pub const DND_ADD_TO_EXISTING: Icon = icon_from_path!("../data/icons/dnd_add_to_existing.svg");
pub const DND_MOVE: Icon = icon_from_path!("../data/icons/dnd_move.svg");
pub const DND_HANDLE: Icon = icon_from_path!("../data/icons/dnd_handle.svg");

/// `>`
pub const BREADCRUMBS_SEPARATOR: Icon = icon_from_path!("../data/icons/breadcrumbs_separator.svg");

pub const SEARCH: Icon = icon_from_path!("../data/icons/search.svg");
pub const SETTINGS: Icon = icon_from_path!("../data/icons/settings.svg");

// Shortcuts:
pub const LEFT_MOUSE_CLICK: Icon = icon_from_path!("../data/icons/lmc.svg");
pub const RIGHT_MOUSE_CLICK: Icon = icon_from_path!("../data/icons/rmc.svg");
pub const SCROLL: Icon = icon_from_path!("../data/icons/scroll.svg");
pub const SHIFT: Icon = icon_from_path!("../data/icons/shift.svg");
pub const CONTROL: Icon = icon_from_path!("../data/icons/control.svg");
pub const COMMAND: Icon = icon_from_path!("../data/icons/command.svg");
pub const OPTION: Icon = icon_from_path!("../data/icons/option.svg");

// Action buttons:
pub const COPY: Icon = icon_from_path!("../data/icons/copy.svg");
pub const DOWNLOAD: Icon = icon_from_path!("../data/icons/download.svg");

// Other non-icon-sized images:
pub const DROPDOWN_ARROW: Icon = icon_from_path!("../data/icons/dropdown_arrow.svg");
pub const CHECKED: Icon = icon_from_path!("../data/icons/checked.svg");
