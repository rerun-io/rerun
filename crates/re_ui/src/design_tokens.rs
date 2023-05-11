use egui::Color32;

/// The look and feel of the UI.
///
/// Not everything is covered by this.
/// A lot of other design tokens are put straight into the [`egui::Style`]
#[derive(Clone, Copy, Debug)]
pub struct DesignTokens {
    pub top_bar_color: egui::Color32,
    pub bottom_bar_color: egui::Color32,
    pub bottom_bar_stroke: egui::Stroke,
    pub bottom_bar_rounding: egui::Rounding,
    pub shadow_gradient_dark_start: egui::Color32,
    pub success_bg_color: egui::Color32,
    pub success_hover_bg_color: egui::Color32,
    pub warning_bg_color: egui::Color32,
    pub warning_hover_bg_color: egui::Color32,
    pub error_bg_color: egui::Color32,
    pub error_hover_bg_color: egui::Color32,
    pub primary_bg_color: egui::Color32,
    pub primary_hover_bg_color: egui::Color32,
    pub gray_50: egui::Color32,
    pub gray_900: egui::Color32,
    pub primary_700: egui::Color32,
}

impl DesignTokens {
    /// Create [`DesignTokens`] and apply style to the given egui context.
    pub fn load_and_apply(ctx: &egui::Context) -> Self {
        apply_design_tokens(ctx)
    }
}

fn apply_design_tokens(ctx: &egui::Context) -> DesignTokens {
    let apply_font = true;
    let apply_font_size = true;

    let json: serde_json::Value = serde_json::from_str(include_str!("../data/design_tokens.json"))
        .expect("Failed to parse data/design_tokens.json");

    let typography_default: Typography = get_alias(&json, "{Alias.Typography.Default.value}");

    if apply_font {
        assert_eq!(typography_default.fontFamily, "Inter");
        assert_eq!(typography_default.fontWeight, "Medium");
        let mut font_definitions = egui::FontDefinitions::default();
        font_definitions.font_data.insert(
            "Inter-Medium".into(),
            egui::FontData::from_static(include_bytes!("../data/Inter-Medium.otf")),
        );
        font_definitions
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "Inter-Medium".into());
        ctx.set_fonts(font_definitions);
    }

    let mut egui_style = egui::Style {
        visuals: egui::Visuals::light(),
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

        // We want labels and buttons to have the same height.
        // Intuitively, we would just assign font_size to
        // the interact_size, but in practice text height does not match
        // font size (for unknown reason), so we fudge it for now:

        egui_style.spacing.interact_size.y = 15.0;
        // egui_style.spacing.interact_size.y = font_size;
    }

    let panel_bg_color = get_aliased_color(&json, "{Alias.Color.Surface.Default.value}");
    let floating_color = get_aliased_color(&json, "{Alias.Color.Surface.Floating.value}");

    // Used as the background of text edits, scroll bars and others things
    // that needs to look different from other interactive stuff.
    // We need this very dark, since the theme overall is very, very dark.
    egui_style.visuals.extreme_bg_color = egui::Color32::WHITE;

    egui_style.visuals.widgets.noninteractive.weak_bg_fill = panel_bg_color;
    egui_style.visuals.widgets.noninteractive.bg_fill = panel_bg_color;

    egui_style.visuals.button_frame = true;
    egui_style.visuals.widgets.inactive.weak_bg_fill =
        get_aliased_color(&json, "{Alias.Color.Action.Inactive.value}"); // Buttons have no background color when inactive
    egui_style.visuals.widgets.inactive.bg_fill =
        get_aliased_color(&json, "{Alias.Color.Action.Default.value}");

    {
        // Background colors for buttons (menu buttons, blueprint buttons, etc) when hovered or clicked:
        let hovered_color = get_aliased_color(&json, "{Alias.Color.Action.Hovered.value}");
        egui_style.visuals.widgets.hovered.weak_bg_fill = hovered_color;
        egui_style.visuals.widgets.hovered.bg_fill = hovered_color;
        egui_style.visuals.widgets.active.weak_bg_fill = hovered_color;
        egui_style.visuals.widgets.active.bg_fill = hovered_color;
        egui_style.visuals.widgets.open.weak_bg_fill = hovered_color;
        egui_style.visuals.widgets.open.bg_fill = hovered_color;
    }

    {
        let border_color = get_global_color(&json, "{Global.Color.Gray.200}");
        egui_style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border_color);
        egui_style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, border_color);
        egui_style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, border_color);
        egui_style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, border_color);
    }

    {
        // Expand hovered and active button frames:
        egui_style.visuals.widgets.hovered.expansion = 2.0;
        egui_style.visuals.widgets.active.expansion = 2.0;
        egui_style.visuals.widgets.open.expansion = 2.0;
    }

    egui_style.visuals.selection.bg_fill =
        get_aliased_color(&json, "{Alias.Color.Highlight.Default.value}");

    egui_style.visuals.widgets.noninteractive.bg_stroke.color = Color32::from_gray(30); // from figma. separator lines, panel lines, etc

    let subudued = get_aliased_color(&json, "{Alias.Color.Text.Subdued.value}");
    let default = get_aliased_color(&json, "{Alias.Color.Text.Default.value}");
    let strong = get_aliased_color(&json, "{Alias.Color.Text.Strong.value}");

    egui_style.visuals.widgets.noninteractive.fg_stroke.color = subudued; // non-interactive text
    egui_style.visuals.widgets.inactive.fg_stroke.color = default; // button text
    egui_style.visuals.widgets.active.fg_stroke.color = strong; // strong text and active button text

    egui_style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    egui_style.visuals.window_shadow = egui::epaint::Shadow::NONE;

    egui_style.visuals.window_fill = floating_color; // tooltips and menus
    egui_style.visuals.window_stroke = egui::Stroke::NONE;
    egui_style.visuals.panel_fill = panel_bg_color;

    egui_style.visuals.window_rounding = crate::ReUi::window_rounding().into();
    egui_style.visuals.menu_rounding = crate::ReUi::window_rounding().into();
    let small_rounding = crate::ReUi::small_rounding().into();
    egui_style.visuals.widgets.noninteractive.rounding = small_rounding;
    egui_style.visuals.widgets.inactive.rounding = small_rounding;
    egui_style.visuals.widgets.hovered.rounding = small_rounding;
    egui_style.visuals.widgets.active.rounding = small_rounding;
    egui_style.visuals.widgets.open.rounding = small_rounding;

    egui_style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    egui_style.spacing.menu_margin = crate::ReUi::view_padding().into();

    // Add stripes to grids and tables?
    egui_style.visuals.striped = false;
    egui_style.visuals.indent_has_left_vline = false;
    egui_style.spacing.button_padding = egui::Vec2::new(12.0, 4.0); // Makes the icons in the blueprint panel align
    egui_style.spacing.indent = 14.0; // From figma

    egui_style.debug.show_blocking_widget = false; // turn this on to debug interaction problems

    egui_style.spacing.combo_width = 8.0; // minimum width of ComboBox - keep them small, with the down-arrow close.

    egui_style.spacing.icon_width = 18.0; // Checkbox width and height
    egui_style.spacing.scroll_bar_inner_margin = 2.0;
    egui_style.spacing.scroll_bar_width = 6.0;
    egui_style.spacing.scroll_bar_outer_margin = 2.0;

    ctx.set_style(egui_style);

    DesignTokens {
        top_bar_color: get_global_color(&json, "{Global.Color.Gray.200}"), // copied from figma
        bottom_bar_color: get_global_color(&json, "{Global.Color.Gray.100}"),
        bottom_bar_stroke: egui::Stroke::new(
            1.0,
            Color32::TRANSPARENT, // Transparent because it doesn't look good in light mode
        ), // copied from figma
        bottom_bar_rounding: egui::Rounding {
            nw: 6.0,
            ne: 6.0,
            sw: 0.0,
            se: 0.0,
        }, // copied from figma, should be top only
        shadow_gradient_dark_start: Color32::TRANSPARENT,
        success_bg_color: get_global_color(&json, "{Global.Color.Success.200}"),
        success_hover_bg_color: get_global_color(&json, "{Global.Color.Success.300}"),
        warning_bg_color: get_global_color(&json, "{Global.Color.Warning.200}"),
        warning_hover_bg_color: get_global_color(&json, "{Global.Color.Warning.300}"),
        error_bg_color: get_global_color(&json, "{Global.Color.Error.200}"),
        error_hover_bg_color: get_global_color(&json, "{Global.Color.Error.300}"),
        primary_bg_color: get_global_color(&json, "{Global.Color.Primary.Default}"),
        primary_hover_bg_color: get_global_color(&json, "{Global.Color.Primary.500}"),
        gray_50: get_global_color(&json, "{Global.Color.Gray.50}"),
        gray_900: get_global_color(&json, "{Global.Color.Gray.900}"),
        primary_700: get_global_color(&json, "{Global.Color.Primary.700}"),
    }
}

// ----------------------------------------------------------------------------

fn get_aliased_color(json: &serde_json::Value, alias_path: &str) -> egui::Color32 {
    re_log::debug!("Alias path: {alias_path}");
    parse_color(get_alias_str(json, alias_path))
}

fn get_global_color(json: &serde_json::Value, global_path: &str) -> egui::Color32 {
    re_log::debug!("Global path: {global_path}");
    parse_color(global_path_value(json, global_path).as_str().unwrap())
}

fn get_alias_str<'json>(json: &'json serde_json::Value, alias_path: &str) -> &'json str {
    let global_path = follow_path_or_die(json, alias_path).as_str().unwrap();
    global_path_value(json, global_path).as_str().unwrap()
}

fn get_alias<T: serde::de::DeserializeOwned>(json: &serde_json::Value, alias_path: &str) -> T {
    let global_path = follow_path_or_die(json, alias_path).as_str().unwrap();
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
    follow_path_or_die(value, global_path).get("value").unwrap()
}

fn follow_path_or_die<'json>(
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
    apply_design_tokens(&ctx);

    // Make sure it works:
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello Test!");
        });
    });
}
