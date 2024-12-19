#![allow(clippy::unwrap_used)]

use crate::color_table::Scale::{S100, S1000, S150, S200, S250, S300, S325, S350, S550, S775};
use crate::color_table::{ColorTable, ColorToken};
use crate::{design_tokens, CUSTOM_WINDOW_DECORATIONS};

/// The look and feel of the UI.
///
/// Not everything is covered by this.
/// A lot of other design tokens are put straight into the [`egui::Style`]
#[derive(Debug)]
pub struct DesignTokens {
    pub json: serde_json::Value,

    /// Color table for all colors used in the UI.
    ///
    /// Loaded at startup from `design_tokens.json`.
    pub color_table: ColorTable,

    // TODO(ab): get rid of these, they should be function calls like the rest.
    pub top_bar_color: egui::Color32,
    pub bottom_bar_color: egui::Color32,
    pub bottom_bar_stroke: egui::Stroke,
    pub bottom_bar_rounding: egui::Rounding,
    pub shadow_gradient_dark_start: egui::Color32,
    pub tab_bar_color: egui::Color32,
    pub native_frame_stroke: egui::Stroke,
}

impl DesignTokens {
    /// Load design tokens from `data/design_tokens.json`.
    pub fn load() -> Self {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../data/design_tokens.json"))
                .expect("Failed to parse data/design_tokens.json");

        let color_table = load_color_table(&json);

        Self {
            top_bar_color: color_table.gray(S100),
            bottom_bar_color: color_table.gray(S150),
            bottom_bar_stroke: egui::Stroke::new(1.0, color_table.gray(S250)),
            bottom_bar_rounding: egui::Rounding {
                nw: 6.0,
                ne: 6.0,
                sw: 0.0,
                se: 0.0,
            }, // copied from figma, should be top only
            shadow_gradient_dark_start: egui::Color32::from_black_alpha(77), //TODO(ab): use ColorToken!
            tab_bar_color: color_table.gray(S200),
            native_frame_stroke: egui::Stroke::new(1.0, color_table.gray(S250)),
            json,
            color_table,
        }
    }

    /// Apply style to the given egui context.
    pub(crate) fn apply(&self, ctx: &egui::Context) {
        let apply_font = true;
        let apply_font_size = true;

        let typography_default: Typography =
            get_alias(&self.json, "{Alias.Typography.Default.value}");

        if apply_font {
            assert_eq!(typography_default.fontFamily, "Inter");
            assert_eq!(typography_default.fontWeight, "Medium");
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

        let mut egui_style = egui::Style {
            visuals: egui::Visuals::dark(),
            ..Default::default()
        };

        if apply_font_size {
            let font_size = parse_px(&typography_default.fontSize);

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
        }

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

        let panel_bg_color = self.color(ColorToken::gray(S100));
        // let floating_color = get_aliased_color(&json, "{Alias.Color.Surface.Floating.value}");
        let floating_color = self.color(ColorToken::gray(S250));

        // For table zebra stripes.
        egui_style.visuals.faint_bg_color = self.color(ColorToken::gray(S150));

        // Used as the background of text edits, scroll bars and others things
        // that needs to look different from other interactive stuff.
        // We need this very dark, since the theme overall is very, very dark.
        egui_style.visuals.extreme_bg_color = egui::Color32::BLACK;

        egui_style.visuals.widgets.noninteractive.weak_bg_fill = panel_bg_color;
        egui_style.visuals.widgets.noninteractive.bg_fill = panel_bg_color;

        egui_style.visuals.button_frame = true;
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

        egui_style.visuals.selection.bg_fill = self.color(ColorToken::blue(S350));

        //TODO(ab): use ColorToken!
        egui_style.visuals.selection.stroke.color = egui::Color32::from_rgb(173, 184, 255); // Brighter version of the above

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
            offset: egui::vec2(0.0, 15.0),
            blur: 50.0,
            spread: 0.0,
            color: egui::Color32::from_black_alpha(128),
        };
        egui_style.visuals.popup_shadow = shadow;
        egui_style.visuals.window_shadow = shadow;

        egui_style.visuals.window_fill = floating_color; // tooltips and menus
        egui_style.visuals.window_stroke = egui::Stroke::NONE;
        egui_style.visuals.panel_fill = panel_bg_color;

        egui_style.visuals.window_rounding = Self::window_rounding().into();
        egui_style.visuals.menu_rounding = Self::window_rounding().into();
        let small_rounding = Self::small_rounding().into();
        egui_style.visuals.widgets.noninteractive.rounding = small_rounding;
        egui_style.visuals.widgets.inactive.rounding = small_rounding;
        egui_style.visuals.widgets.hovered.rounding = small_rounding;
        egui_style.visuals.widgets.active.rounding = small_rounding;
        egui_style.visuals.widgets.open.rounding = small_rounding;

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

        // don't color hyperlinks #2733
        egui_style.visuals.hyperlink_color = default;

        egui_style.visuals.image_loading_spinners = false;

        //TODO(#8333): use ColorToken!
        egui_style.visuals.error_fg_color = egui::Color32::from_rgb(0xAB, 0x01, 0x16);
        egui_style.visuals.warn_fg_color = egui::Color32::from_rgb(0xFF, 0x7A, 0x0C);

        ctx.set_style(egui_style);
    }

    /// Get the [`egui::Color32`] corresponding to the provided [`ColorToken`].
    #[inline]
    pub fn color(&self, token: ColorToken) -> egui::Color32 {
        self.color_table.get(token)
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
    pub fn view_padding() -> f32 {
        12.0
    }

    pub fn panel_margin() -> egui::Margin {
        egui::Margin::symmetric(Self::view_padding(), 0.0)
    }

    pub fn window_rounding() -> f32 {
        12.0
    }

    pub fn normal_rounding() -> f32 {
        6.0
    }

    pub fn small_rounding() -> f32 {
        4.0
    }

    pub fn table_line_height() -> f32 {
        20.0 // should be big enough to contain buttons, i.e. egui_style.spacing.interact_size.y
    }

    pub fn table_header_height() -> f32 {
        20.0
    }

    pub fn top_bar_margin() -> egui::Margin {
        egui::Margin::symmetric(8.0, 2.0)
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

    pub fn native_window_rounding() -> f32 {
        10.0
    }

    pub fn top_panel_frame() -> egui::Frame {
        let mut frame = egui::Frame {
            inner_margin: Self::top_bar_margin(),
            fill: design_tokens().top_bar_color,
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.rounding.nw = Self::native_window_rounding();
            frame.rounding.ne = Self::native_window_rounding();
        }
        frame
    }

    pub fn bottom_panel_margin() -> egui::Margin {
        Self::top_bar_margin()
    }

    /// For the streams view (time panel)
    pub fn bottom_panel_frame() -> egui::Frame {
        // Show a stroke only on the top. To achieve this, we add a negative outer margin.
        // (on the inner margin we counteract this again)
        let margin_offset = design_tokens().bottom_bar_stroke.width * 0.5;

        let margin = Self::bottom_panel_margin();

        let design_tokens = design_tokens();

        let mut frame = egui::Frame {
            fill: design_tokens.bottom_bar_color,
            inner_margin: margin + margin_offset,
            outer_margin: egui::Margin {
                left: -margin_offset,
                right: -margin_offset,
                // Add a proper stoke width thick margin on the top.
                top: design_tokens.bottom_bar_stroke.width,
                bottom: -margin_offset,
            },
            stroke: design_tokens.bottom_bar_stroke,
            rounding: design_tokens.bottom_bar_rounding,
            ..Default::default()
        };
        if CUSTOM_WINDOW_DECORATIONS {
            frame.rounding.sw = Self::native_window_rounding();
            frame.rounding.se = Self::native_window_rounding();
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

    /// The color for the background of [`crate::SectionCollapsingHeader`].
    pub fn section_collapsing_header_color(&self) -> egui::Color32 {
        // same as visuals.widgets.inactive.bg_fill
        self.color(ColorToken::gray(S200))
    }

    /// The color we use to mean "loop this selection"
    pub fn loop_selection_color() -> egui::Color32 {
        egui::Color32::from_rgb(1, 37, 105) // from figma 2023-02-09
    }

    /// The color we use to mean "loop all the data"
    pub fn loop_everything_color() -> egui::Color32 {
        egui::Color32::from_rgb(2, 80, 45) // from figma 2023-02-09
    }

    /// Used by the "add view or container" modal.
    pub fn thumbnail_background_color(&self) -> egui::Color32 {
        self.color(ColorToken::gray(S250))
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
    fn get_color_from_json(json: &serde_json::Value, global_path: &str) -> egui::Color32 {
        parse_color(global_path_value(json, global_path).as_str().unwrap())
    }

    ColorTable::new(|color_token| {
        get_color_from_json(
            json,
            &format!("{{Global.Color.{}.{}}}", color_token.hue, color_token.scale),
        )
    })
}

fn get_alias<T: serde::de::DeserializeOwned>(json: &serde_json::Value, alias_path: &str) -> T {
    let global_path = follow_path_or_panic(json, alias_path).as_str().unwrap();
    let global_value = global_path_value(json, global_path);
    serde_json::from_value(global_value.clone()).unwrap_or_else(|err| {
        panic!(
            "Failed to convert {global_path:?} to {}: {err}. Json: {json:?}",
            std::any::type_name::<T>()
        )
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
#[derive(serde::Deserialize)]
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

fn parse_color(color: &str) -> egui::Color32 {
    #![allow(clippy::identity_op)]

    let color = color.strip_prefix('#').unwrap();
    if color.len() == 6 {
        // RGB
        let color = u32::from_str_radix(color, 16).unwrap();
        egui::Color32::from_rgb(
            ((color >> 16) & 0xff) as u8,
            ((color >> 8) & 0xff) as u8,
            ((color >> 0) & 0xff) as u8,
        )
    } else if color.len() == 8 {
        // RGBA
        let color = u32::from_str_radix(color, 16).unwrap();
        egui::Color32::from_rgba_unmultiplied(
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
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello Test!");
        });
    });
}
