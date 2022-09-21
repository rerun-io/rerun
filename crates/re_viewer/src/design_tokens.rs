pub(crate) fn apply_design_tokens(ctx: &egui::Context) {
    let apply_font = true;
    let apply_font_size = true;

    let json: serde_json::Value = serde_json::from_str(include_str!("../data/design_tokens.json"))
        .expect("Failed to parse data/design_tokens.json");

    let typography_default: Typography =
        get_alias(&json, &["Alias", "Typography", "Default", "value"]);

    if apply_font {
        assert_eq!(typography_default.fontFamily, "Inter");
        assert_eq!(typography_default.fontWeight, "Medium");
        let mut font_definitions = egui::FontDefinitions::default();
        font_definitions.font_data.insert(
            "Inter-Medium".into(),
            egui::FontData::from_static(include_bytes!("../data/Inter-Medium.ttf")),
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
    }

    egui_style.visuals.widgets.noninteractive.bg_fill =
        get_aliased_color(&json, &["Alias", "Color", "Surface", "Default", "value"]);
    // TODO(emilk): window top bars

    egui_style.visuals.widgets.inactive.bg_fill =
        get_aliased_color(&json, &["Alias", "Color", "Action", "Default", "value"]);

    egui_style.visuals.widgets.hovered.bg_fill =
        get_aliased_color(&json, &["Alias", "Color", "Action", "Hovered", "value"]);

    egui_style.visuals.widgets.noninteractive.fg_stroke.color =
        get_aliased_color(&json, &["Alias", "Color", "Text", "Default", "value"]);
    egui_style.visuals.widgets.active.fg_stroke.color =
        get_aliased_color(&json, &["Alias", "Color", "Text", "Strong", "value"]);
    // egui_style.visuals.widgets.inactive.fg_stroke.color =
    //     get_aliased_color(&json, &["Alias", "Color", "Icon", "Default", "value"]);
    // TODO(emilk): weak text color

    ctx.set_style(egui_style);
}

fn get_aliased_color(json: &serde_json::Value, path: &[&str]) -> egui::Color32 {
    parse_color(get_alias_str(json, path))
}

fn get_alias_str<'json>(json: &'json serde_json::Value, path: &[&str]) -> &'json str {
    let global_path = get(json, path).as_str().unwrap();
    look_up_or_die(json, global_path).as_str().unwrap()
}

fn get_alias<T: serde::de::DeserializeOwned>(json: &serde_json::Value, path: &[&str]) -> T {
    let global_path = get(json, path).as_str().unwrap();
    let global_value = look_up_or_die(json, global_path);
    serde_json::from_value(global_value.clone()).unwrap_or_else(|err| {
        panic!(
            "Failed to convert {global_path:?} to {}: {err}. Json: {json:?}",
            std::any::type_name::<T>()
        )
    })
}

fn get<'json>(mut json: &'json serde_json::Value, path: &[&str]) -> &'json serde_json::Value {
    for component in path {
        json = json
            .get(component)
            .unwrap_or_else(|| panic!("Failed to follow json path {path:?}"));
    }
    json
}

#[allow(non_snake_case)]
#[derive(serde::Deserialize)]
struct Typography {
    fontSize: String,
    fontWeight: String,
    fontFamily: String,
    // lineHeight: String,  // TODO(emilk)
    // letterSpacing: String, // TODO(emilk)
}

fn look_up_or_die<'json>(value: &'json serde_json::Value, path: &str) -> &'json serde_json::Value {
    let json =
        look_up(value, path).unwrap_or_else(|| panic!("Failed to find {path:?} in design tokens"));
    json.get("value").unwrap()
}

fn look_up<'json>(
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
