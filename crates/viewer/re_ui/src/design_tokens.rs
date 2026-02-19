#![expect(clippy::unwrap_used)]
#![expect(clippy::unused_self)] // TODO(emilk): move hard-coded values into .ron files

use anyhow::Context as _;
use egui::{Color32, Margin, Stroke, Theme, Vec2};

use crate::color_table::{ColorTable, ColorToken, Hue, Scale};
use crate::{CUSTOM_WINDOW_DECORATIONS, format_with_decimals_in_range};

#[derive(Debug)]
pub struct AlertVisuals {
    pub fill: Color32,
    pub icon: Color32,
    pub text: Color32,
}

impl AlertVisuals {
    fn try_get(color_table: &ColorTable, ron: &ron::Value, name: &str) -> anyhow::Result<Self> {
        let value = ron.get(name)?;

        Ok(Self {
            fill: color_from_json(color_table, value.get("fill")?)?,
            icon: color_from_json(color_table, value.get("icon")?)?,
            text: color_from_json(color_table, value.get("text")?)?,
        })
    }

    fn get(color_table: &ColorTable, ron: &ron::Value, name: &str) -> Self {
        Self::try_get(color_table, ron, name).expect("Failed to parse AlertVisuals")
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum TableStyle {
    /// Used for presenting a lot of information to the user
    /// without wasting vertical space, like when showing log output.
    #[default]
    Dense,

    /// Used when we want to fit big, clickable buttons in the table cells.
    Spacious,
}

/// The look and feel of the UI.
///
/// Not everything is covered by this.
/// A lot of other design tokens are put straight into the [`egui::Style`]
#[derive(Debug)]
pub struct DesignTokens {
    pub theme: egui::Theme,

    typography: Typography,

    pub large_button_size: Vec2,
    pub large_button_icon_size: Vec2,
    pub large_button_corner_radius: f32,
    pub small_icon_size: Vec2,
    pub modal_button_width: f32,
    pub default_modal_width: f32,

    // All these colors can be found in dark_theme.ron and light_theme.ron:
    pub top_bar_color: Color32,
    pub bottom_bar_color: Color32,
    pub bottom_bar_stroke: Stroke,
    pub shadow_gradient_dark_start: Color32,
    pub tab_bar_color: Color32,
    pub native_frame_stroke: Stroke,

    /// Usually black or white
    pub strong_fg_color: Color32,

    pub info_log_text_color: Color32,
    pub debug_log_text_color: Color32,
    pub trace_log_text_color: Color32,

    pub success_text_color: Color32,
    pub info_text_color: Color32,

    /// Opacity multiplier for the background of 2D labels in spatial views.
    pub spatial_label_bg_opacity: f32,

    /// Background color for viewport views.
    pub viewport_background: Color32,

    /// Background color for widgets that should catch the user's attention.
    pub highlight_color: Color32,

    /// Color of an icon next to a label
    pub label_button_icon_color: Color32,

    /// The color for the background of [`crate::SectionCollapsingHeader`].
    pub section_header_color: Color32,

    /// The color we use to mean "loop this selection"
    pub loop_selection_color: Color32,

    /// Like [`Self::loop_selection_color`], but inactive.
    pub loop_selection_color_inactive: Color32,

    /// The color we use to mean "loop all the data"
    pub loop_everything_color: Color32,

    /// Color for thumbnail backgrounds
    pub thumbnail_background_color: Color32,

    /// Color for example card backgrounds
    pub example_card_background_color: Color32,

    pub example_tag_bg_fill: Color32,
    pub example_tag_stroke: Stroke,

    // ------
    // Colors for things with a blue selection-background ("primary"), e.g. the breadcrumb:
    pub surface_on_primary_hovered: Color32,
    pub text_color_on_primary: Color32,
    pub text_color_on_primary_hovered: Color32,
    pub icon_color_on_primary: Color32,
    pub icon_color_on_primary_hovered: Color32,
    pub selection_stroke_color: Color32,
    pub selection_bg_fill: Color32,
    pub focus_outline_stroke: Stroke,
    pub focus_halo_stroke: Stroke,

    // ------
    pub panel_bg_color: Color32,

    pub text_edit_bg_color: Color32,

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

    pub table_interaction_row_selection_fill: Color32,

    pub table_sort_icon_color: Color32,

    pub drag_pill_droppable_fill: Color32,
    pub drag_pill_droppable_stroke: Color32,
    pub drag_pill_nondroppable_fill: Color32,
    pub drag_pill_nondroppable_stroke: Color32,

    /// Stroke used to indicate that a UI element is a container that will receive a drag-and-drop
    /// payload.
    ///
    /// Sometimes this is the UI element that is being dragged over (e.g., a view receiving a new
    /// entity). Sometimes this is a UI element not under the pointer, but whose content is
    /// being hovered (e.g., a container in the blueprint tree)
    pub drop_target_container_stroke: Stroke,

    /// When drag-and-dropping a tile, the candidate area is drawn with this stroke.
    pub tile_drag_preview_stroke: Stroke,

    /// When drag-and-dropping a tile, the candidate area is drawn with this background color.
    pub tile_drag_preview_color: Color32,

    pub floating_color: Color32,
    pub faint_bg_color: Color32,
    pub extreme_bg_color: Color32,
    pub extreme_fg_color: Color32,
    pub widget_inactive_bg_fill: Color32,
    pub widget_hovered_color: Color32,
    pub widget_hovered_weak_bg_fill: Color32,
    pub widget_hovered_bg_fill: Color32,
    pub widget_active_weak_bg_fill: Color32,
    pub widget_active_bg_fill: Color32,
    pub widget_open_weak_bg_fill: Color32,
    pub widget_noninteractive_weak_bg_fill: Color32,
    pub widget_noninteractive_bg_fill: Color32,
    pub widget_noninteractive_bg_stroke: Color32,
    pub text_subdued: Color32,
    pub text_default: Color32,
    pub text_strong: Color32,
    pub error_fg_color: Color32,
    pub warn_fg_color: Color32,
    pub popup_shadow_color: Color32,

    pub alert_success: AlertVisuals,
    pub alert_info: AlertVisuals,
    pub alert_warning: AlertVisuals,
    pub alert_error: AlertVisuals,

    pub density_graph_selected: Color32,
    pub density_graph_unselected: Color32,

    /// This is the color of time ranges that has only been partially loaded.
    pub density_graph_outside_valid_ranges: Color32,

    // Spatial view colors:
    pub axis_color_x: Color32,
    pub axis_color_y: Color32,
    pub axis_color_z: Color32,
    pub frustum_color: Color32,

    // List item colors
    pub list_item_active_text: Color32,
    pub list_item_noninteractive_text: Color32,
    pub list_item_hovered_text: Color32,
    pub list_item_default_text: Color32,
    pub list_item_strong_text: Color32,
    pub list_item_active_icon: Color32,
    pub list_item_hovered_icon: Color32,
    pub list_item_default_icon: Color32,
    pub list_item_hovered_bg: Color32,
    pub list_item_active_bg: Color32,
    pub list_item_collapse_default: Color32,

    // Visualizer list (selection panel)
    pub visualizer_list_title_text_color: Color32,
    pub visualizer_list_path_text_color: Color32,
    pub visualizer_list_color_box_size: f32,
    pub visualizer_list_color_box_stroke: Stroke,
    pub visualizer_list_pill_bg_color: Color32,
    pub visualizer_list_pill_bg_color_hovered: Color32,

    pub code_index_color: Color32,
    pub code_string_color: Color32,
    pub code_null_color: Color32,
    pub code_primitive_color: Color32,
    pub code_keyword_color: Color32,

    // Table filter UI
    pub table_filter_frame_stroke: Stroke,

    pub bg_fill_inverse: Color32,
    pub bg_fill_inverse_hover: Color32,
    pub text_inverse: Color32,
    pub icon_inverse: Color32,
}

impl DesignTokens {
    /// Load design tokens from `data/design_tokens_*.ron`.
    pub fn load(theme: Theme, tokens_ron: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(!tokens_ron.trim().is_empty(), "Empty theme file");

        let color_table_ron: ron::Value = ron::from_str(include_str!("../data/color_table.ron"))
            .expect("Failed to parse data/color_table.ron");
        let colors = load_color_table(&color_table_ron);

        let theme_json: ron::Value = ron::from_str(tokens_ron)
            .with_context(|| format!("Failed to parse {theme:?} theme .ron"))?;

        let typography: Typography = parse_path(&theme_json, "{Global.Typography.Default}");

        let get_scalar = |scalar_name: &str| try_get_scalar(&theme_json, scalar_name);
        let get_color = |color_name: &str| get_aliased_color(&colors, &theme_json, color_name);
        let get_stroke = |stroke_name: &str| get_aliased_stroke(&colors, &theme_json, stroke_name);

        let selection_bg_fill = get_color("selection_bg_fill");

        let loop_selection_color =
            selection_bg_fill.gamma_multiply(get_scalar("loop_selection_alpha")?);
        let loop_selection_color_inactive =
            selection_bg_fill.gamma_multiply(get_scalar("loop_selection_alpha_inactive")?);

        Ok(Self {
            theme,
            typography,

            large_button_size: Vec2::splat(get_scalar("large_button_size")?),
            large_button_icon_size: Vec2::splat(get_scalar("large_button_icon_size")?),
            large_button_corner_radius: get_scalar("large_button_corner_radius")?,
            small_icon_size: Vec2::splat(get_scalar("small_icon_size")?),
            modal_button_width: get_scalar("modal_button_width")?,
            default_modal_width: get_scalar("default_modal_width")?,

            top_bar_color: get_color("top_bar_color"),
            bottom_bar_color: get_color("bottom_bar_color"),
            bottom_bar_stroke: get_stroke("bottom_bar_stroke"),
            shadow_gradient_dark_start: get_color("shadow_gradient_dark_start"),
            tab_bar_color: get_color("tab_bar_color"),
            native_frame_stroke: get_stroke("native_frame_stroke"),
            strong_fg_color: get_color("strong_fg_color"),

            info_log_text_color: get_color("info_log_text_color"),
            debug_log_text_color: get_color("debug_log_text_color"),
            trace_log_text_color: get_color("trace_log_text_color"),

            success_text_color: get_color("success_text_color"),
            info_text_color: get_color("info_text_color"),

            spatial_label_bg_opacity: get_scalar("spatial_label_bg_opacity")?,

            viewport_background: get_color("viewport_background"),

            highlight_color: get_color("highlight_color"),

            label_button_icon_color: get_color("label_button_icon_color"),
            section_header_color: get_color("section_header_color"),

            loop_selection_color,
            loop_selection_color_inactive,
            loop_everything_color: get_color("loop_everything_color"),

            thumbnail_background_color: get_color("thumbnail_background_color"),

            example_card_background_color: get_color("example_card_background_color"),
            example_tag_bg_fill: get_color("example_tag_bg_fill"),
            example_tag_stroke: get_stroke("example_tag_stroke"),

            surface_on_primary_hovered: get_color("surface_on_primary_hovered"),
            text_color_on_primary: get_color("text_color_on_primary"),
            text_color_on_primary_hovered: get_color("text_color_on_primary_hovered"),
            icon_color_on_primary: get_color("icon_color_on_primary"),
            icon_color_on_primary_hovered: get_color("icon_color_on_primary_hovered"),
            selection_bg_fill,
            selection_stroke_color: get_color("selection_stroke_color"),
            focus_outline_stroke: get_stroke("focus_outline_stroke"),
            focus_halo_stroke: get_stroke("focus_halo_stroke"),

            panel_bg_color: get_color("panel_bg_color"),
            text_edit_bg_color: get_color("text_edit_bg_color"),
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
            table_interaction_row_selection_fill: get_color("table_interaction_row_selection_fill"),
            table_sort_icon_color: get_color("table_sort_icon_color"),

            drag_pill_droppable_fill: get_color("drag_pill_droppable_fill"),
            drag_pill_droppable_stroke: get_color("drag_pill_droppable_stroke"),
            drag_pill_nondroppable_fill: get_color("drag_pill_nondroppable_fill"),
            drag_pill_nondroppable_stroke: get_color("drag_pill_nondroppable_stroke"),
            drop_target_container_stroke: get_stroke("drop_target_container_stroke"),
            tile_drag_preview_stroke: get_stroke("tile_drag_preview_stroke"),
            tile_drag_preview_color: get_color("tile_drag_preview_color"),

            floating_color: get_color("floating_color"),
            faint_bg_color: get_color("faint_bg_color"),
            extreme_bg_color: get_color("extreme_bg_color"),
            extreme_fg_color: get_color("extreme_fg_color"),
            widget_inactive_bg_fill: get_color("widget_inactive_bg_fill"),
            widget_hovered_color: get_color("widget_hovered_color"),
            widget_hovered_weak_bg_fill: get_color("widget_hovered_weak_bg_fill"),
            widget_hovered_bg_fill: get_color("widget_hovered_bg_fill"),
            widget_active_weak_bg_fill: get_color("widget_active_weak_bg_fill"),
            widget_active_bg_fill: get_color("widget_active_bg_fill"),
            widget_open_weak_bg_fill: get_color("widget_open_weak_bg_fill"),
            widget_noninteractive_weak_bg_fill: get_color("widget_noninteractive_weak_bg_fill"),
            widget_noninteractive_bg_fill: get_color("widget_noninteractive_bg_fill"),
            widget_noninteractive_bg_stroke: get_color("widget_noninteractive_bg_stroke"),
            text_subdued: get_color("text_subdued"),
            text_default: get_color("text_default"),
            text_strong: get_color("text_strong"),
            error_fg_color: get_color("error_fg_color"),
            warn_fg_color: get_color("warn_fg_color"),

            alert_success: AlertVisuals::get(&colors, &theme_json, "alert_success"),
            alert_info: AlertVisuals::get(&colors, &theme_json, "alert_info"),
            alert_warning: AlertVisuals::get(&colors, &theme_json, "alert_warning"),
            alert_error: AlertVisuals::get(&colors, &theme_json, "alert_error"),

            popup_shadow_color: get_color("popup_shadow_color"),

            density_graph_selected: get_color("density_graph_selected"),
            density_graph_unselected: get_color("density_graph_unselected"),
            density_graph_outside_valid_ranges: get_color("density_graph_outside_valid_ranges"),

            axis_color_x: get_color("axis_color_x"),
            axis_color_y: get_color("axis_color_y"),
            axis_color_z: get_color("axis_color_z"),
            frustum_color: get_color("frustum_color"),

            // List item colors
            list_item_active_text: get_color("list_item_active_text"),
            list_item_noninteractive_text: get_color("list_item_noninteractive_text"),
            list_item_hovered_text: get_color("list_item_hovered_text"),
            list_item_default_text: get_color("list_item_default_text"),
            list_item_strong_text: get_color("list_item_strong_text"),
            list_item_active_icon: get_color("list_item_active_icon"),
            list_item_hovered_icon: get_color("list_item_hovered_icon"),
            list_item_default_icon: get_color("list_item_default_icon"),
            list_item_hovered_bg: get_color("list_item_hovered_bg"),
            list_item_active_bg: get_color("list_item_active_bg"),
            list_item_collapse_default: get_color("list_item_collapse_default"),

            visualizer_list_title_text_color: get_color("visualizer_list_title_text_color"),
            visualizer_list_path_text_color: get_color("visualizer_list_path_text_color"),
            visualizer_list_color_box_size: get_scalar("visualizer_list_color_box_size")?,
            visualizer_list_color_box_stroke: get_stroke("visualizer_list_color_box_stroke"),
            visualizer_list_pill_bg_color: get_color("visualizer_list_pill_bg_color"),
            visualizer_list_pill_bg_color_hovered: get_color(
                "visualizer_list_pill_bg_color_hovered",
            ),

            code_index_color: get_color("code_index_color"),
            code_string_color: get_color("code_string_color"),
            code_null_color: get_color("code_null_color"),
            code_primitive_color: get_color("code_primitive_color"),

            code_keyword_color: get_color("code_keyword_color"),
            table_filter_frame_stroke: get_stroke("table_filter_frame_stroke"),

            bg_fill_inverse: get_color("bg_fill_inverse"),
            bg_fill_inverse_hover: get_color("bg_fill_inverse-hover"),
            text_inverse: get_color("text_inverse"),
            icon_inverse: get_color("icon_inverse"),
        })
    }

    /// Apply style to the given egui context.
    pub(crate) fn apply(&self, style: &mut egui::Style) {
        re_tracing::profile_function!();

        self.set_text_styles(style);
        self.set_spacing(style);
        self.set_colors(style);

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

    fn set_spacing(&self, egui_style: &mut egui::Style) {
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

        egui_style.visuals.window_corner_radius = self.window_corner_radius().into();
        egui_style.visuals.menu_corner_radius = self.window_corner_radius().into();
        let small_corner_radius = self.small_corner_radius().into();
        egui_style.visuals.widgets.noninteractive.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.inactive.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.hovered.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.active.corner_radius = small_corner_radius;
        egui_style.visuals.widgets.open.corner_radius = small_corner_radius;

        egui_style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        egui_style.spacing.menu_margin = self.view_padding().into();
        egui_style.spacing.menu_spacing = 1.0;

        // avoid some visual glitches with the default non-zero value
        egui_style.visuals.clip_rect_margin = 0.0;

        // Add stripes to grids and tables?
        egui_style.visuals.striped = false;
        egui_style.visuals.indent_has_left_vline = false;
        egui_style.spacing.button_padding = Vec2::new(1.0, 0.0); // Makes the icons in the blueprint panel align
        egui_style.spacing.indent = 14.0; // From figma

        egui_style.spacing.combo_width = 8.0; // minimum width of ComboBox - keep them small, with the down-arrow close.

        egui_style.spacing.scroll.bar_inner_margin = 2.0;
        egui_style.spacing.scroll.bar_width = 6.0;
        egui_style.spacing.scroll.bar_outer_margin = 2.0;

        egui_style.spacing.tooltip_width = 600.0;

        egui_style.visuals.image_loading_spinners = false;
    }

    fn set_colors(&self, egui_style: &mut egui::Style) {
        // For table zebra stripes.
        egui_style.visuals.faint_bg_color = self.faint_bg_color;

        // Used as the background of scroll bars and others things
        // that needs to look different from other interactive stuff..
        egui_style.visuals.extreme_bg_color = self.extreme_bg_color;

        egui_style.visuals.widgets.noninteractive.weak_bg_fill = self.panel_bg_color;
        egui_style.visuals.widgets.noninteractive.bg_fill = self.panel_bg_color;
        egui_style.visuals.text_edit_bg_color = Some(self.text_edit_bg_color);

        egui_style.visuals.widgets.inactive.weak_bg_fill = Default::default(); // Buttons have no background color when inactive

        // Fill of unchecked radio buttons, checkboxes, etc. Must be brighter than the background floating_color.
        egui_style.visuals.widgets.inactive.bg_fill = self.widget_inactive_bg_fill;

        {
            // Background colors for buttons (menu buttons, blueprint buttons, etc) when hovered or clicked:
            let hovered_color = self.widget_hovered_color;
            egui_style.visuals.widgets.hovered.weak_bg_fill = hovered_color;
            egui_style.visuals.widgets.hovered.bg_fill = hovered_color;
            egui_style.visuals.widgets.active.weak_bg_fill = hovered_color;
            egui_style.visuals.widgets.active.bg_fill = hovered_color;
            egui_style.visuals.widgets.open.weak_bg_fill = hovered_color;
            egui_style.visuals.widgets.open.bg_fill = hovered_color;
        }

        egui_style.visuals.selection.bg_fill = self.selection_bg_fill;
        egui_style.visuals.selection.stroke.color = self.selection_stroke_color;

        // separator lines, panel lines, etc
        egui_style.visuals.widgets.noninteractive.bg_stroke.color =
            self.widget_noninteractive_bg_stroke;

        let subdued = self.text_subdued;
        let default = self.text_default;
        let strong = self.text_strong;

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
            color: self.popup_shadow_color,
        };
        egui_style.visuals.popup_shadow = shadow;
        egui_style.visuals.window_shadow = shadow;

        egui_style.visuals.window_fill = self.floating_color; // tooltips and menus
        egui_style.visuals.window_stroke = Stroke::NONE;
        egui_style.visuals.panel_fill = self.panel_bg_color;

        // don't color hyperlinks #2733
        egui_style.visuals.hyperlink_color = default;

        egui_style.visuals.error_fg_color = self.error_fg_color;
        egui_style.visuals.warn_fg_color = self.warn_fg_color;
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
    pub fn view_padding(&self) -> i8 {
        12
    }

    pub fn panel_margin(&self) -> egui::Margin {
        egui::Margin::symmetric(self.view_padding(), 0)
    }

    pub fn menu_button_padding() -> f32 {
        6.0
    }

    pub fn window_corner_radius(&self) -> u8 {
        6
    }

    pub fn normal_corner_radius(&self) -> u8 {
        6
    }

    pub fn small_corner_radius(&self) -> u8 {
        4
    }

    pub fn table_cell_margin(&self, table_style: TableStyle) -> Margin {
        match table_style {
            TableStyle::Dense => Margin::symmetric(8, 2),
            TableStyle::Spacious => Margin::symmetric(8, 6),
        }
    }

    /// The total row height, including margin/spacing.
    pub fn table_row_height(&self, table_style: TableStyle) -> f32 {
        match table_style {
            TableStyle::Dense => 20.0,

            // Should be big enough to contain buttons, i.e. egui_style.spacing.interact_size.y
            // and the cell margin.
            TableStyle::Spacious => 32.0,
        }
    }

    /// The max height of the content.
    pub fn table_content_height(&self, table_style: TableStyle) -> f32 {
        self.table_row_height(table_style) - self.table_cell_margin(table_style).sum().y
    }

    pub fn header_cell_margin(&self, _table_style: TableStyle) -> Margin {
        Margin::symmetric(8, 6)
    }

    pub fn table_header_height(&self) -> f32 {
        32.0
    }

    // TODO(lucasmerlin): Update all tables to the new design
    pub fn deprecated_table_header_height(&self) -> f32 {
        20.0
    }

    pub fn top_bar_margin(&self) -> egui::Margin {
        egui::Margin::symmetric(8, 0)
    }

    pub fn text_to_icon_padding(&self) -> f32 {
        4.0
    }

    /// Height of the top-most bar.
    pub fn top_bar_height(&self) -> f32 {
        28.0 // Don't waste vertical space, especially important for embedded web viewers
    }

    /// Height of the title row in the blueprint view and selection view,
    /// as well as the tab bar height in the viewport view.
    pub fn title_bar_height(&self) -> f32 {
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

    pub fn combo_item_max_value_width() -> f32 {
        124.0
    }

    pub fn combo_item_small_font_size() -> f32 {
        10.0
    }

    pub fn native_window_corner_radius(&self) -> u8 {
        10
    }

    pub fn top_panel_frame(&self) -> egui::Frame {
        let mut frame = egui::Frame {
            inner_margin: self.top_bar_margin(),
            fill: self.top_bar_color,
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.corner_radius.nw = self.native_window_corner_radius();
            frame.corner_radius.ne = self.native_window_corner_radius();
        }
        frame
    }

    /// Something that provides contrast vs the background
    pub fn popup_frame(&self, style: &egui::Style) -> egui::Frame {
        egui::Frame::window(style)
            .fill(self.notification_panel_background_color)
            .corner_radius(8)
            .inner_margin(8.0)
    }

    pub fn bottom_panel_margin(&self) -> egui::Margin {
        self.top_bar_margin()
    }

    /// For the streams view (time panel)
    pub fn bottom_panel_frame(&self) -> egui::Frame {
        // Show a stroke only on the top. To achieve this, we add a negative outer margin.
        // (on the inner margin we counteract this again)
        let margin_offset = (self.bottom_bar_stroke.width * 0.5) as i8;

        let margin = self.bottom_panel_margin();

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
            corner_radius: 0.0.into(),
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.corner_radius.sw = self.native_window_corner_radius();
            frame.corner_radius.se = self.native_window_corner_radius();
        }
        frame
    }

    pub fn setup_table_header(_header: &mut egui_extras::TableRow<'_, '_>) {}

    pub fn setup_table_body(&self, body: &mut egui_extras::TableBody<'_>, table_style: TableStyle) {
        // Make sure buttons don't visually overflow:
        body.ui_mut().spacing_mut().interact_size.y = self.table_content_height(table_style);

        // No extra spacing between items in the table body - we bake that into the row height.
        body.ui_mut().spacing_mut().item_spacing.y = 0.0;
    }

    /// Layout area to allocate for the collapsing triangle.
    ///
    /// Note that this is not the _size_ of the collapsing triangle (which is defined by
    /// [`crate::UiExt::paint_collapsing_triangle`]), but how much screen real-estate should be
    /// allocated for it. It's set to the same size as the small icon size so that everything is
    /// properly aligned in [`crate::list_item::ListItem`].
    pub fn collapsing_triangle_size(&self) -> Vec2 {
        self.small_icon_size
    }
}

// ----------------------------------------------------------------------------

trait RonExt {
    /// Supports path-like access to the JSON structure.
    fn get(&self, path: &str) -> anyhow::Result<&Self> {
        let mut value = self;
        for component in path.split('.') {
            if let Some(child) = value.get_child(component) {
                value = child;
            } else {
                anyhow::bail!("Failed to find {component:?} in path {path:?}");
            }
        }
        Ok(value)
    }

    fn get_child(&self, key: &str) -> Option<&Self>;

    fn as_str(&self) -> Option<&str>;

    fn as_f32(&self) -> Option<f32>;

    fn as_u8(&self) -> Option<u8> {
        let value = self.as_f32()?;
        if value as u8 as f32 == value {
            Some(value as u8)
        } else {
            None
        }
    }
}

impl RonExt for ron::Value {
    fn get_child(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Map(map) => map.get(&Self::String(key.into())),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Number(i) => Some(i.into_f64() as f32),
            _ => None,
        }
    }
}

/// Build the [`ColorTable`] based on the content of `design_token.ron`
fn load_color_table(json: &ron::Value) -> ColorTable {
    fn get_color_from_json(json: &ron::Value, global_path: &str) -> Color32 {
        Color32::from_hex(global_path_value(json, global_path).as_str().unwrap()).unwrap()
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
    json: &ron::Value,
    color_name: &str,
) -> anyhow::Result<Color32> {
    let color_alias = json.get("Alias")?.get(color_name)?;
    color_from_json(color_table, color_alias)
}

fn color_from_json(color_table: &ColorTable, color_alias: &ron::Value) -> anyhow::Result<Color32> {
    let color = color_alias
        .get("color")?
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("color not a string"))?;

    let mut color = if color.starts_with('#') {
        Color32::from_hex(color)
            .map_err(|color_error| anyhow::anyhow!("Invalid hex color: {color_error:?}"))?
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
        color_table.get(ColorToken::new(hue, scale))
    } else {
        anyhow::bail!("Expected {{hue.scale}} or #RRGGBB")
    };

    if let Ok(alpha) = color_alias.get("alpha") {
        let alpha = alpha
            .as_u8()
            .ok_or_else(|| anyhow::anyhow!("alpha should be an integer 0-255"))?;
        color = color.gamma_multiply_u8(alpha);
    }

    Ok(color)
}

fn try_get_scalar(json: &ron::Value, path: &str) -> anyhow::Result<f32> {
    json.get(path)?
        .as_f32()
        .ok_or_else(|| anyhow::anyhow!("'{path}' not a number"))
}

#[expect(clippy::panic)]
fn get_aliased_color(color_table: &ColorTable, json: &ron::Value, alias_path: &str) -> Color32 {
    try_get_alias_color(color_table, json, alias_path).unwrap_or_else(|err| {
        panic!("Failed to get aliased color at {alias_path:?}: {err}");
    })
}

#[expect(clippy::panic)]
fn get_aliased_stroke(color_table: &ColorTable, json: &ron::Value, alias_path: &str) -> Stroke {
    try_get_aliased_stroke(color_table, json, alias_path).unwrap_or_else(|err| {
        panic!("Failed to get aliased stroke at {alias_path:?}: {err}");
    })
}

fn try_get_aliased_stroke(
    color_table: &ColorTable,
    json: &ron::Value,
    alias_path: &str,
) -> anyhow::Result<Stroke> {
    let color_alias = json.get("Alias")?.get(alias_path)?;

    let color = color_from_json(color_table, color_alias)?;
    let width = color_alias
        .get("width")?
        .as_f32()
        .ok_or_else(|| anyhow::anyhow!("'Alias.{alias_path}.width' not a number"))?;
    let stroke = Stroke::new(width, color);
    Ok(stroke)
}

fn global_path_value<'json>(value: &'json ron::Value, global_path: &str) -> &'json ron::Value {
    follow_path_or_panic(value, global_path)
        .get("value")
        .unwrap()
}

#[expect(clippy::panic)]
fn parse_path<T: serde::de::DeserializeOwned>(json: &ron::Value, global_path: &str) -> T {
    let global_value = global_path_value(json, global_path);
    global_value.clone().into_rust().unwrap_or_else(|err| {
        panic!(
            "Failed to convert {global_path:?} to {}: {err}. Json: {json:?}",
            std::any::type_name::<T>()
        )
    })
}

#[expect(clippy::panic)]
fn follow_path_or_panic<'json>(json: &'json ron::Value, json_path: &str) -> &'json ron::Value {
    follow_path(json, json_path).unwrap_or_else(|| panic!("Failed to find {json_path:?}"))
}

fn follow_path<'json>(mut value: &'json ron::Value, path: &str) -> Option<&'json ron::Value> {
    let path = path.strip_prefix('{')?;
    let path = path.strip_suffix('}')?;
    for component in path.split('.') {
        value = value.get_child(component)?;
    }
    Some(value)
}

// ----------------------------------------------------------------------------

#[expect(non_snake_case)]
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
