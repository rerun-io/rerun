#![allow(clippy::unwrap_used)]
#![allow(clippy::enum_glob_use)] // Nice to have for the color variants

use anyhow::Context as _;
use egui::{Color32, Theme};

use crate::{
    CUSTOM_WINDOW_DECORATIONS, Hue, Scale,
    color_table::{ColorTable, ColorToken, Scale::*},
    format_with_decimals_in_range,
};

/// The look and feel of the UI.
///
/// Not everything is covered by this.
/// A lot of other design tokens are put straight into the [`egui::Style`]
#[derive(Debug)]
pub struct DesignTokens {
    pub theme: egui::Theme,

    typography: Typography,

    /// Color table for all colors used in the UI.
    ///
    /// Loaded at startup from `color_table.json`.
    pub(crate) color_table: ColorTable, // TODO: remove

    // All these colors can be found in dark_theme.json and light_theme.json:
    pub top_bar_color: Color32,
    pub bottom_bar_color: Color32,
    pub bottom_bar_stroke: egui::Stroke,
    pub bottom_bar_corner_radius: egui::CornerRadius,
    pub shadow_gradient_dark_start: Color32,
    pub tab_bar_color: Color32,
    pub native_frame_stroke: egui::Stroke,
    pub strong_fg_color: Color32,
    pub info_log_text_color: Color32,
    pub debug_log_text_color: Color32,
    pub trace_log_text_color: Color32,

    /// Color of an icon next to a label
    pub label_button_icon_color: Color32,

    /// The color for the background of [`crate::SectionCollapsingHeader`].
    pub section_collapsing_header_color: Color32,

    /// The color we use to mean "loop this selection"
    pub loop_selection_color: Color32,

    /// The color we use to mean "loop all the data"
    pub loop_everything_color: Color32,

    /// Color for thumbnail backgrounds
    pub thumbnail_background_color: Color32,

    /// Color for example card backgrounds
    pub example_card_background_color: Color32,

    /// Color for breadcrumb text
    pub breadcrumb_text_color: Color32,

    /// Color for breadcrumb separators
    pub breadcrumb_separator_color: Color32,

    /// Color for blueprint time panel background
    pub blueprint_time_panel_bg_fill: Color32,

    /// Color for notification panel background
    pub notification_panel_background_color: Color32,

    /// Color for notification background
    pub notification_background_color: Color32,

    /// Color for table header background
    pub table_header_bg_fill: Color32,

    /// Color for table header stroke
    pub table_header_stroke_color: Color32,

    /// Color for table interaction hovered background stroke
    pub table_interaction_hovered_bg_stroke: Color32,

    /// Color for table interaction active background stroke
    pub table_interaction_active_bg_stroke: Color32,

    /// Color for table interaction noninteractive background stroke
    pub table_interaction_noninteractive_bg_stroke: Color32,

    pub drag_pill_droppable_fill: Color32,
    pub drag_pill_droppable_stroke: Color32,
    pub drag_pill_nondroppable_fill: Color32,
    pub drag_pill_nondroppable_stroke: Color32,
}

impl DesignTokens {
    /// Load design tokens from `data/design_tokens_*.json`.
    pub fn load(theme: Theme) -> Self {
        let color_table_json: serde_json::Value =
            serde_json::from_str(include_str!("../data/color_table.json"))
                .expect("Failed to parse data/color_table.json");
        let colors = load_color_table(&color_table_json);

        let theme_json: serde_json::Value = match theme {
            egui::Theme::Dark => serde_json::from_str(include_str!("../data/dark_theme.json"))
                .expect("Failed to parse data/dark_theme.json"),

            egui::Theme::Light => serde_json::from_str(include_str!("../data/light_theme.json"))
                .expect("Failed to parse data/light_theme.json"),
        };

        let typography: Typography = parse_path(&theme_json, "{Global.Typography.Default}");

        let get_color = |color_name: &str| get_aliased_color(&colors, &theme_json, color_name);

        let top_bar_color = get_color("top_bar_color");
        let tab_bar_color = get_color("tab_bar_color");
        let bottom_bar_color = get_color("bottom_bar_color");
        let bottom_bar_stroke_color = get_color("bottom_bar_stroke_color");
        let shadow_gradient_dark_start = get_color("shadow_gradient_dark_start");
        let native_frame_stroke_color = get_color("native_frame_stroke_color");

        Self {
            theme,
            typography,
            top_bar_color,
            bottom_bar_color,
            bottom_bar_stroke: egui::Stroke::new(1.0, bottom_bar_stroke_color),
            bottom_bar_corner_radius: egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 0,
                se: 0,
            }, // copied from figma, should be top only
            shadow_gradient_dark_start,
            tab_bar_color,
            native_frame_stroke: egui::Stroke::new(1.0, native_frame_stroke_color),
            strong_fg_color: get_color("strong_fg_color"),

            info_log_text_color: get_color("info_log_text_color"),
            debug_log_text_color: get_color("debug_log_text_color"),
            trace_log_text_color: get_color("trace_log_text_color"),

            label_button_icon_color: get_color("label_button_icon_color"),
            section_collapsing_header_color: get_color("section_collapsing_header_color"),

            loop_selection_color: get_color("loop_selection_color"),
            loop_everything_color: get_color("loop_everything_color"),

            thumbnail_background_color: get_color("thumbnail_background_color"),
            example_card_background_color: get_color("example_card_background_color"),
            breadcrumb_text_color: get_color("breadcrumb_text_color"),
            breadcrumb_separator_color: get_color("breadcrumb_separator_color"),
            blueprint_time_panel_bg_fill: get_color("blueprint_time_panel_bg_fill"),
            notification_panel_background_color: get_color("notification_panel_background_color"),
            notification_background_color: get_color("notification_background_color"),
            table_header_bg_fill: get_color("table_header_bg_fill"),
            table_header_stroke_color: get_color("table_header_stroke_color"),
            table_interaction_hovered_bg_stroke: get_color("table_interaction_hovered_bg_stroke"),
            table_interaction_active_bg_stroke: get_color("table_interaction_active_bg_stroke"),
            table_interaction_noninteractive_bg_stroke: get_color(
                "table_interaction_noninteractive_bg_stroke",
            ),

            drag_pill_droppable_fill: get_color("drag_pill_droppable_fill"),
            drag_pill_droppable_stroke: get_color("drag_pill_droppable_stroke"),
            drag_pill_nondroppable_fill: get_color("drag_pill_nondroppable_fill"),
            drag_pill_nondroppable_stroke: get_color("drag_pill_nondroppable_stroke"),

            color_table: colors,
        }
    }

    /// Apply style to the given egui context.
    pub(crate) fn apply(&self, style: &mut egui::Style) {
        re_tracing::profile_function!();

        self.set_text_styles(style);
        Self::set_common_style(style);

        match self.theme {
            egui::Theme::Dark => {
                self.set_dark_style(style);
            }
            egui::Theme::Light => {
                self.set_light_style(style);
            }
        }

        style.number_formatter = egui::style::NumberFormatter::new(format_with_decimals_in_range);
    }

    pub(crate) fn set_fonts(&self, ctx: &egui::Context) {
        assert_eq!(self.typography.fontFamily, "Inter");
        assert_eq!(self.typography.fontWeight, "Medium");
        let mut font_definitions = egui::FontDefinitions::default();
        font_definitions.font_data.insert(
            "Inter-Medium".into(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../data/Inter-Medium.otf"
            ))),
        );
        font_definitions
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "Inter-Medium".into());
        ctx.set_fonts(font_definitions);
    }

    /// Get the [`Color32`] corresponding to the provided [`ColorToken`].
    // TODO: make private
    #[inline]
    pub fn color(&self, token: ColorToken) -> Color32 {
        self.color_table.get(token)
    }

    fn set_text_styles(&self, egui_style: &mut egui::Style) {
        let font_size = parse_px(&self.typography.fontSize);

        for text_style in [
            egui::TextStyle::Body,
            egui::TextStyle::Monospace,
            egui::TextStyle::Button,
        ] {
            egui_style.text_styles.get_mut(&text_style).unwrap().size = font_size;
        }

        egui_style
            .text_styles
            .get_mut(&egui::TextStyle::Heading)
            .unwrap()
            .size = 16.0;

        // We want labels and buttons to have the same height.
        // Intuitively, we would just assign font_size to
        // the interact_size, but in practice text height does not match
        // font size (for unknown reason), so we fudge it for now:

        egui_style.spacing.interact_size.y = 15.0;
        // egui_style.spacing.interact_size.y = font_size;

        // fonts used in the welcome screen
        // TODO(ab): font sizes should come from design tokens
        egui_style
            .text_styles
            .insert(Self::welcome_screen_h1(), egui::FontId::proportional(41.0));
        egui_style
            .text_styles
            .insert(Self::welcome_screen_h2(), egui::FontId::proportional(27.0));
        egui_style.text_styles.insert(
            Self::welcome_screen_example_title(),
            egui::FontId::proportional(13.0),
        );
        egui_style.text_styles.insert(
            Self::welcome_screen_body(),
            egui::FontId::proportional(15.0),
        );
        egui_style
            .text_styles
            .insert(Self::welcome_screen_tag(), egui::FontId::proportional(10.5));
    }

    fn set_common_style(egui_style: &mut egui::Style) {
        egui_style.visuals.button_frame = true;

        {
            // Turn off strokes around buttons:
            egui_style.visuals.widgets.inactive.bg_stroke = Default::default();
            egui_style.visuals.widgets.hovered.bg_stroke = Default::default();
            egui_style.visuals.widgets.active.bg_stroke = Default::default();
            egui_style.visuals.widgets.open.bg_stroke = Default::default();
        }

        {
            egui_style.visuals.widgets.hovered.expansion = 2.0;
            egui_style.visuals.widgets.active.expansion = 2.0;
            egui_style.visuals.widgets.open.expansion = 2.0;
        }

        egui_style.visuals.window_corner_radius = Self::window_corner_radius().into();
        egui_style.visuals.menu_corner_radius = Self::window_corner_radius().into();
        let small_corner_radius = Self::small_corner_radius().into();
        egui_style.visuals.widgets.noninteractive.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.inactive.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.hovered.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.active.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.open.corner_radius = small_corner_radius;

        egui_style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        egui_style.spacing.menu_margin = Self::view_padding().into();
        egui_style.spacing.menu_spacing = 1.0;

        // avoid some visual glitches with the default non-zero value
        egui_style.visuals.clip_rect_margin = 0.0;

        // Add stripes to grids and tables?
        egui_style.visuals.striped = false;
        egui_style.visuals.indent_has_left_vline = false;
        egui_style.spacing.button_padding = egui::Vec2::new(1.0, 0.0); // Makes the icons in the blueprint panel align
        egui_style.spacing.indent = 14.0; // From figma

        egui_style.spacing.combo_width = 8.0; // minimum width of ComboBox - keep them small, with the down-arrow close.

        egui_style.spacing.scroll.bar_inner_margin = 2.0;
        egui_style.spacing.scroll.bar_width = 6.0;
        egui_style.spacing.scroll.bar_outer_margin = 2.0;

        egui_style.spacing.tooltip_width = 720.0;

        egui_style.visuals.image_loading_spinners = false;
    }

    fn set_dark_style(&self, egui_style: &mut egui::Style) {
        let panel_bg_color = self.color(ColorToken::gray(S100));
        // let floating_color = get_aliased_color(&json, "{Alias.Color.Surface.Floating.value}");
        let floating_color = self.color(ColorToken::gray(S250));

        // For table zebra stripes.
        egui_style.visuals.faint_bg_color = self.color(ColorToken::gray(S150));

        // Used as the background of text edits, scroll bars and others things
        // that needs to look different from other interactive stuff.
        // We need this very dark, since the theme overall is very, very dark.
        egui_style.visuals.extreme_bg_color = Color32::BLACK;

        egui_style.visuals.widgets.noninteractive.weak_bg_fill = panel_bg_color;
        egui_style.visuals.widgets.noninteractive.bg_fill = panel_bg_color;

        egui_style.visuals.widgets.inactive.weak_bg_fill = Default::default(); // Buttons have no background color when inactive

        // Fill of unchecked radio buttons, checkboxes, etc. Must be brighter than the background floating_color.
        egui_style.visuals.widgets.inactive.bg_fill = self.color(ColorToken::gray(S300));

        {
            // Background colors for buttons (menu buttons, blueprint buttons, etc) when hovered or clicked:
            let hovered_color = self.color(ColorToken::gray(S325));
            egui_style.visuals.widgets.hovered.weak_bg_fill = hovered_color;
            egui_style.visuals.widgets.hovered.bg_fill = hovered_color;
            egui_style.visuals.widgets.active.weak_bg_fill = hovered_color;
            egui_style.visuals.widgets.active.bg_fill = hovered_color;
            egui_style.visuals.widgets.open.weak_bg_fill = hovered_color;
            egui_style.visuals.widgets.open.bg_fill = hovered_color;
        }

        egui_style.visuals.selection.bg_fill = self.color(ColorToken::blue(S350));

        //TODO(ab): use ColorToken!
        egui_style.visuals.selection.stroke.color = Color32::from_rgb(173, 184, 255); // Brighter version of the above

        // separator lines, panel lines, etc
        egui_style.visuals.widgets.noninteractive.bg_stroke.color =
            self.color(ColorToken::gray(S250));

        let subdued = self.color(ColorToken::gray(S550));
        let default = self.color(ColorToken::gray(S775));
        let strong = self.color(ColorToken::gray(S1000));

        egui_style.visuals.widgets.noninteractive.fg_stroke.color = subdued; // non-interactive text
        egui_style.visuals.widgets.inactive.fg_stroke.color = default; // button text
        egui_style.visuals.widgets.active.fg_stroke.color = strong; // strong text and active button text

        let wide_stroke_width = 2.0; // Make it a bit more visible, especially important for spatial primitives.
        egui_style.visuals.widgets.active.fg_stroke.width = wide_stroke_width;
        egui_style.visuals.selection.stroke.width = wide_stroke_width;

        // From figma
        let shadow = egui::epaint::Shadow {
            offset: [0, 15],
            blur: 50,
            spread: 0,
            color: Color32::from_black_alpha(128),
        };
        egui_style.visuals.popup_shadow = shadow;
        egui_style.visuals.window_shadow = shadow;

        egui_style.visuals.window_fill = floating_color; // tooltips and menus
        egui_style.visuals.window_stroke = egui::Stroke::NONE;
        egui_style.visuals.panel_fill = panel_bg_color;

        // don't color hyperlinks #2733
        egui_style.visuals.hyperlink_color = default;

        //TODO(#8333): use ColorToken!
        egui_style.visuals.error_fg_color = Color32::from_rgb(0xAB, 0x01, 0x16);
        egui_style.visuals.warn_fg_color = Color32::from_rgb(0xFF, 0x7A, 0x0C);
    }

    fn set_light_style(&self, egui_style: &mut egui::Style) {
        let panel_bg_color = self.color(ColorToken::gray(S1000));
        // let floating_color = get_aliased_color(&json, "{Alias.Color.Surface.Floating.value}");
        let floating_color = self.color(ColorToken::gray(S1000));

        // For table zebra stripes.
        egui_style.visuals.faint_bg_color = self.color(ColorToken::gray(S900));

        // Used as the background of text edits, scroll bars and others things
        // that needs to look different from other interactive stuff.
        // We need this very dark, since the theme overall is very, very dark.
        egui_style.visuals.extreme_bg_color = self.color(ColorToken::gray(S900));

        egui_style.visuals.widgets.noninteractive.weak_bg_fill = self.color(ColorToken::gray(S550));
        egui_style.visuals.widgets.noninteractive.bg_fill = self.color(ColorToken::gray(S800));

        egui_style.visuals.widgets.inactive.weak_bg_fill = Default::default(); // Buttons have no background color when inactive

        // Fill of unchecked radio buttons, checkboxes, etc. Must be brighter than the background floating_color.
        egui_style.visuals.widgets.inactive.bg_fill = self.color(ColorToken::gray(S800));

        {
            // Background colors for buttons (menu buttons, blueprint buttons, etc) when hovered or clicked:
            // defaulted to egui light theme for now
            //let hovered_color = egui::Color32::RED;
            egui_style.visuals.widgets.hovered.weak_bg_fill = self.color(ColorToken::gray(S800));
            egui_style.visuals.widgets.hovered.bg_fill = self.color(ColorToken::gray(S700));
            egui_style.visuals.widgets.active.weak_bg_fill = self.color(ColorToken::gray(S750)); //ms after weak_bg_fill is pressed
            egui_style.visuals.widgets.active.bg_fill = self.color(ColorToken::gray(S650)); //ms after btn on timeline is pressed
            egui_style.visuals.widgets.open.weak_bg_fill = self.color(ColorToken::gray(S650));
            //when dropdown is opened
            //egui_style.visuals.widgets.open.bg_fill = egui::Color32::GREEN; //have no idea when its triggered
        }

        egui_style.visuals.selection.bg_fill = self.color(ColorToken::blue(S550));
        egui_style.visuals.selection.stroke.color = self.color(ColorToken::blue(S800)); // Brighter version of the above

        // separator lines, panel lines, etc
        egui_style.visuals.widgets.noninteractive.bg_stroke.color =
            self.color(ColorToken::gray(S800));

        let subdued = self.color(ColorToken::gray(S500));
        let default = self.color(ColorToken::gray(S250));
        let strong = self.color(ColorToken::gray(S0));

        egui_style.visuals.widgets.noninteractive.fg_stroke.color = subdued; // non-interactive text
        egui_style.visuals.widgets.inactive.fg_stroke.color = default; // button text
        egui_style.visuals.widgets.active.fg_stroke.color = strong; // strong text and active button text

        let wide_stroke_width = 2.0; // Make it a bit more visible, especially important for spatial primitives.
        egui_style.visuals.widgets.active.fg_stroke.width = wide_stroke_width;
        egui_style.visuals.selection.stroke.width = wide_stroke_width;

        // From figma
        let shadow = egui::epaint::Shadow {
            offset: [0, 15],
            blur: 50,
            spread: 0,
            color: Color32::from_black_alpha(32),
        };
        egui_style.visuals.popup_shadow = shadow;
        egui_style.visuals.window_shadow = shadow;

        egui_style.visuals.window_fill = floating_color; // tooltips and menus
        egui_style.visuals.window_stroke = egui::Stroke::NONE;
        egui_style.visuals.panel_fill = panel_bg_color;

        // don't color hyperlinks #2733
        egui_style.visuals.hyperlink_color = default;

        //TODO(#8333): use ColorToken!
        egui_style.visuals.error_fg_color = Color32::from_rgb(0xAB, 0x01, 0x16);
        egui_style.visuals.warn_fg_color = Color32::from_rgb(0xFF, 0x7A, 0x0C);
    }

    #[inline]
    pub fn welcome_screen_h1() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-h1".into())
    }

    #[inline]
    pub fn welcome_screen_h2() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-h2".into())
    }

    #[inline]
    pub fn welcome_screen_example_title() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-example-title".into())
    }

    #[inline]
    pub fn welcome_screen_body() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-body".into())
    }

    #[inline]
    pub fn welcome_screen_tag() -> egui::TextStyle {
        egui::TextStyle::Name("welcome-screen-tag".into())
    }

    /// Margin on all sides of views.
    pub fn view_padding() -> i8 {
        12
    }

    pub fn panel_margin() -> egui::Margin {
        egui::Margin::symmetric(Self::view_padding(), 0)
    }

    pub fn window_corner_radius() -> f32 {
        12.0
    }

    pub fn normal_corner_radius() -> f32 {
        6.0
    }

    pub fn small_corner_radius() -> f32 {
        4.0
    }

    pub fn table_line_height() -> f32 {
        20.0 // should be big enough to contain buttons, i.e. egui_style.spacing.interact_size.y
    }

    pub fn table_header_height() -> f32 {
        20.0
    }

    pub fn top_bar_margin() -> egui::Margin {
        egui::Margin::symmetric(8, 0)
    }

    pub fn text_to_icon_padding() -> f32 {
        4.0
    }

    /// Height of the top-most bar.
    pub fn top_bar_height() -> f32 {
        28.0 // Don't waste vertical space, especially important for embedded web viewers
    }

    /// Height of the title row in the blueprint view and selection view,
    /// as well as the tab bar height in the viewport view.
    pub fn title_bar_height() -> f32 {
        24.0 // https://github.com/rerun-io/rerun/issues/5589
    }

    pub fn list_item_height() -> f32 {
        24.0
    }

    pub fn list_header_vertical_offset() -> f32 {
        2.0
    }

    pub fn list_header_font_size() -> f32 {
        11.0
    }

    pub fn native_window_corner_radius() -> u8 {
        10
    }

    pub fn top_panel_frame(&self) -> egui::Frame {
        let mut frame = egui::Frame {
            inner_margin: Self::top_bar_margin(),
            fill: self.top_bar_color,
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.corner_radius.nw = Self::native_window_corner_radius();
            frame.corner_radius.ne = Self::native_window_corner_radius();
        }
        frame
    }

    pub fn bottom_panel_margin() -> egui::Margin {
        Self::top_bar_margin()
    }

    /// For the streams view (time panel)
    pub fn bottom_panel_frame(&self) -> egui::Frame {
        // Show a stroke only on the top. To achieve this, we add a negative outer margin.
        // (on the inner margin we counteract this again)
        let margin_offset = (self.bottom_bar_stroke.width * 0.5) as i8;

        let margin = Self::bottom_panel_margin();

        let mut frame = egui::Frame {
            fill: self.bottom_bar_color,
            inner_margin: margin + margin_offset,
            outer_margin: egui::Margin {
                left: -margin_offset,
                right: -margin_offset,
                // Add a proper stoke width thick margin on the top.
                top: self.bottom_bar_stroke.width as i8,
                bottom: -margin_offset,
            },
            stroke: self.bottom_bar_stroke,
            corner_radius: self.bottom_bar_corner_radius,
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.corner_radius.sw = Self::native_window_corner_radius();
            frame.corner_radius.se = Self::native_window_corner_radius();
        }
        frame
    }

    pub fn small_icon_size() -> egui::Vec2 {
        egui::Vec2::splat(14.0)
    }

    pub fn setup_table_header(_header: &mut egui_extras::TableRow<'_, '_>) {}

    pub fn setup_table_body(body: &mut egui_extras::TableBody<'_>) {
        // Make sure buttons don't visually overflow:
        body.ui_mut().spacing_mut().interact_size.y = Self::table_line_height();
    }

    /// Layout area to allocate for the collapsing triangle.
    ///
    /// Note that this is not the _size_ of the collapsing triangle (which is defined by
    /// [`crate::UiExt::paint_collapsing_triangle`]), but how much screen real-estate should be
    /// allocated for it. It's set to the same size as the small icon size so that everything is
    /// properly aligned in [`crate::list_item::ListItem`].
    pub fn collapsing_triangle_area() -> egui::Vec2 {
        Self::small_icon_size()
    }

    /// Stroke used to indicate that a UI element is a container that will receive a drag-and-drop
    /// payload.
    ///
    /// Sometimes this is the UI element that is being dragged over (e.g., a view receiving a new
    /// entity). Sometimes this is a UI element not under the pointer, but whose content is
    /// being hovered (e.g., a container in the blueprint tree)
    #[inline]
    pub fn drop_target_container_stroke(&self) -> egui::Stroke {
        egui::Stroke::new(2.0, self.color(ColorToken::blue(S350)))
    }

    pub fn text(&self, text: impl Into<String>, token: ColorToken) -> egui::RichText {
        egui::RichText::new(text).color(self.color(token))
    }
}

// ----------------------------------------------------------------------------

/// Build the [`ColorTable`] based on the content of `design_token.json`
fn load_color_table(json: &serde_json::Value) -> ColorTable {
    fn get_color_from_json(json: &serde_json::Value, global_path: &str) -> Color32 {
        parse_color(global_path_value(json, global_path).as_str().unwrap())
    }

    ColorTable::new(|color_token| {
        get_color_from_json(
            json,
            &format!("{{Global.Color.{}.{}}}", color_token.hue, color_token.scale),
        )
    })
}

fn try_get_alias_color(
    color_table: &ColorTable,
    json: &serde_json::Value,
    color_name: &str,
) -> anyhow::Result<Color32> {
    let color_alias = json
        .get("Alias")
        .ok_or_else(|| anyhow::anyhow!("Missing 'Alias'"))?
        .get(color_name)
        .ok_or_else(|| anyhow::anyhow!("Missing 'Alias.{color_name}'"))?;
    let color = color_alias
        .get("color")
        .ok_or_else(|| anyhow::anyhow!("No color found"))?
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("color not a string"))?;

    if color.starts_with('#') {
        Ok(
            Color32::from_hex(color)
                .map_err(|err| anyhow::anyhow!("Invalid hex color: {err:?}"))?,
        )
    } else if color.starts_with('{') {
        let color = color
            .strip_prefix('{')
            .ok_or_else(|| anyhow::anyhow!("Expected {{hue.scale}}"))?;
        let color = color
            .strip_suffix('}')
            .ok_or_else(|| anyhow::anyhow!("Expected {{hue.scale}}"))?;
        let (hue, scale) = color
            .split_once('.')
            .ok_or_else(|| anyhow::anyhow!("Expected {{hue.scale}}"))?;
        let hue: Hue = hue.parse()?;
        let scale: Scale = scale.parse()?;
        let mut color = color_table.get(ColorToken::new(hue, scale));
        if let Some(alpha) = color_alias.get("alpha") {
            let alpha = alpha
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("alpha should be 0-255"))?;
            let alpha: u8 = u8::try_from(alpha).context("alpha should be 0-255")?;
            color = color.gamma_multiply_u8(alpha);
        }
        Ok(color)
    } else {
        anyhow::bail!("Expected {{hue.scale}} or #RRGGBB")
    }
}

fn get_aliased_color(
    color_table: &ColorTable,
    json: &serde_json::Value,
    alias_path: &str,
) -> Color32 {
    try_get_alias_color(color_table, json, alias_path).unwrap_or_else(|err| {
        panic!("Failed to get aliased color at {alias_path:?}: {err}");
    })
}

fn global_path_value<'json>(
    value: &'json serde_json::Value,
    global_path: &str,
) -> &'json serde_json::Value {
    follow_path_or_panic(value, global_path)
        .get("value")
        .unwrap()
}

fn parse_path<T: serde::de::DeserializeOwned>(json: &serde_json::Value, global_path: &str) -> T {
    let global_value = global_path_value(json, global_path);
    serde_json::from_value(global_value.clone()).unwrap_or_else(|err| {
        panic!(
            "Failed to convert {global_path:?} to {}: {err}. Json: {json:?}",
            std::any::type_name::<T>()
        )
    })
}

fn follow_path_or_panic<'json>(
    json: &'json serde_json::Value,
    json_path: &str,
) -> &'json serde_json::Value {
    follow_path(json, json_path).unwrap_or_else(|| panic!("Failed to find {json_path:?}"))
}

fn follow_path<'json>(
    mut value: &'json serde_json::Value,
    path: &str,
) -> Option<&'json serde_json::Value> {
    let path = path.strip_prefix('{')?;
    let path = path.strip_suffix('}')?;
    for component in path.split('.') {
        value = value.get(component)?;
    }
    Some(value)
}

// ----------------------------------------------------------------------------

#[allow(non_snake_case)]
#[derive(Debug, serde::Deserialize)]
struct Typography {
    fontSize: String,
    fontWeight: String,
    fontFamily: String,
    // lineHeight: String,  // TODO(emilk)
    // letterSpacing: String, // TODO(emilk)
}

fn parse_px(pixels: &str) -> f32 {
    pixels.strip_suffix("px").unwrap().parse().unwrap()
}

fn parse_color(color: &str) -> Color32 {
    #![allow(clippy::identity_op)]

    let color = color.strip_prefix('#').unwrap();
    if color.len() == 6 {
        // RGB
        let color = u32::from_str_radix(color, 16).unwrap();
        Color32::from_rgb(
            ((color >> 16) & 0xff) as u8,
            ((color >> 8) & 0xff) as u8,
            ((color >> 0) & 0xff) as u8,
        )
    } else if color.len() == 8 {
        // RGBA
        let color = u32::from_str_radix(color, 16).unwrap();
        Color32::from_rgba_unmultiplied(
            ((color >> 24) & 0xff) as u8,
            ((color >> 16) & 0xff) as u8,
            ((color >> 8) & 0xff) as u8,
            ((color >> 0) & 0xff) as u8,
        )
    } else {
        panic!()
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_design_tokens() {
    let ctx = egui::Context::default();
    crate::apply_style_and_install_loaders(&ctx);

    // Make sure it works:
    let _ignored = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello Test!");
        });
    });
}
